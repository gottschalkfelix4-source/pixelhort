//! Machine-learning jobs. Inference runs in the separate `pixelhort-ml`
//! service (possibly on another machine); this module sends preview images
//! there and stores the results.

use pgvector::Vector;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::model::{AssetRow, get_asset};
use crate::state::SharedState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedFace {
    pub bounding_box: BoundingBox,
    pub score: f32,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Deserialize)]
pub struct OcrResult {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ObjectLabel {
    pub label: String,
    pub score: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictResponse {
    pub image_width: u32,
    pub image_height: u32,
    pub clip: Option<Vec<f32>>,
    pub faces: Option<Vec<DetectedFace>>,
    pub ocr: Option<OcrResult>,
    pub objects: Option<Vec<ObjectLabel>>,
}

fn ml_request(state: &SharedState, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match &state.config.ml_api_key {
        Some(k) => req.header("x-ml-key", k),
        None => req,
    }
}

pub async fn predict(state: &SharedState, asset: &AssetRow, tasks: &str) -> anyhow::Result<PredictResponse> {
    let url = state.ml_url().ok_or_else(|| anyhow::anyhow!("machine learning is disabled"))?;
    let preview = asset.preview_path.as_ref().ok_or_else(|| anyhow::anyhow!("no preview yet"))?;
    let bytes = tokio::fs::read(preview).await?;
    let sys = state.sys();
    let form = reqwest::multipart::Form::new()
        .part("image", reqwest::multipart::Part::bytes(bytes).file_name("preview.jpg").mime_str("image/jpeg")?)
        .text("tasks", tasks.to_string())
        .text("faceMinScore", sys.ml.face_min_score.to_string());
    let res = ml_request(state, state.http.post(format!("{url}/predict")).multipart(form))
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await?
        .error_for_status()?;
    Ok(res.json().await?)
}

pub async fn clip_text(state: &SharedState, text: &str) -> anyhow::Result<Vec<f32>> {
    let key = text.trim().to_lowercase();
    if let Some(v) = state.clip_text_cache.lock().expect("cache").get(&key) {
        return Ok(v.clone());
    }
    let url = state.ml_url().ok_or_else(|| anyhow::anyhow!("machine learning is disabled"))?;
    #[derive(Deserialize)]
    struct R {
        embedding: Vec<f32>,
    }
    let r: R = ml_request(state, state.http.post(format!("{url}/clip/text")).json(&json!({ "text": text })))
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let mut cache = state.clip_text_cache.lock().expect("cache");
    if cache.len() > 500 {
        cache.clear();
    }
    cache.insert(key, r.embedding.clone());
    Ok(r.embedding)
}

async fn mark(state: &SharedState, asset_id: Uuid, column: &str) -> sqlx::Result<()> {
    sqlx::query(&format!("UPDATE assets SET {column} = now() WHERE id = $1"))
        .bind(asset_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn smart_search(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let asset = get_asset(&state.db, asset_id).await?;
    let res = predict(state, &asset, "clip").await?;
    let emb = res.clip.ok_or_else(|| anyhow::anyhow!("no clip embedding returned"))?;
    sqlx::query(
        "INSERT INTO smart_search (asset_id, embedding) VALUES ($1, $2)
         ON CONFLICT (asset_id) DO UPDATE SET embedding = EXCLUDED.embedding",
    )
    .bind(asset_id)
    .bind(Vector::from(emb))
    .execute(&state.db)
    .await?;
    mark(state, asset_id, "smart_search_at").await?;
    state.jobs.enqueue_asset("duplicates", asset_id).await?;
    Ok(())
}

pub async fn detect_faces(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let asset = get_asset(&state.db, asset_id).await?;
    let res = predict(state, &asset, "faces").await?;
    let faces = res.faces.unwrap_or_default();
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM faces WHERE asset_id = $1").bind(asset_id).execute(&mut *tx).await?;
    for face in &faces {
        if face.embedding.len() != 512 {
            continue;
        }
        let b = &face.bounding_box;
        sqlx::query(
            "INSERT INTO faces (id, asset_id, owner_id, x1, y1, x2, y2, image_width, image_height, score, embedding)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(Uuid::new_v4())
        .bind(asset_id)
        .bind(asset.owner_id)
        .bind(b.x1.round() as i32)
        .bind(b.y1.round() as i32)
        .bind(b.x2.round() as i32)
        .bind(b.y2.round() as i32)
        .bind(res.image_width as i32)
        .bind(res.image_height as i32)
        .bind(face.score)
        .bind(Vector::from(face.embedding.clone()))
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE assets SET faces_at = now() WHERE id = $1").bind(asset_id).execute(&mut *tx).await?;
    tx.commit().await?;
    if !faces.is_empty() {
        state.jobs.enqueue_asset("facial-recognition", asset_id).await?;
    }
    Ok(())
}

/// Clusters faces into people: a face joins the person of its nearest
/// recognized neighbour; when enough similar unassigned faces exist a new
/// person is created for all of them.
pub async fn recognize_faces(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let sys = state.sys();
    let max_distance = sys.ml.face_max_distance as f64;
    let min_faces = sys.ml.face_min_faces.max(1);
    let faces: Vec<(Uuid, Uuid, Vector)> = sqlx::query_as(
        "SELECT id, owner_id, embedding FROM faces WHERE asset_id = $1 AND person_id IS NULL",
    )
    .bind(asset_id)
    .fetch_all(&state.db)
    .await?;
    for (face_id, owner_id, embedding) in faces {
        // Re-check: an earlier iteration may have assigned this face already.
        let still_free: Option<Option<Uuid>> = sqlx::query_scalar("SELECT person_id FROM faces WHERE id = $1")
            .bind(face_id)
            .fetch_optional(&state.db)
            .await?;
        if !matches!(still_free, Some(None)) {
            continue;
        }
        let neighbours: Vec<(Uuid, Option<Uuid>, f64)> = sqlx::query_as(
            "SELECT id, person_id, (embedding <=> $1)::float8 AS dist FROM faces
             WHERE owner_id = $2 AND id <> $3 AND (embedding <=> $1) <= $4
             ORDER BY embedding <=> $1 LIMIT 64",
        )
        .bind(&embedding)
        .bind(owner_id)
        .bind(face_id)
        .bind(max_distance)
        .fetch_all(&state.db)
        .await?;

        if let Some(person) = neighbours.iter().find_map(|(_, p, _)| *p) {
            sqlx::query("UPDATE faces SET person_id = $2 WHERE id = $1")
                .bind(face_id)
                .bind(person)
                .execute(&state.db)
                .await?;
            continue;
        }
        if (neighbours.len() as i64) + 1 >= min_faces {
            let person = Uuid::new_v4();
            let mut tx = state.db.begin().await?;
            sqlx::query("INSERT INTO people (id, owner_id, feature_face_id) VALUES ($1, $2, $3)")
                .bind(person)
                .bind(owner_id)
                .bind(face_id)
                .execute(&mut *tx)
                .await?;
            let mut ids: Vec<Uuid> = neighbours.iter().map(|(id, _, _)| *id).collect();
            ids.push(face_id);
            sqlx::query("UPDATE faces SET person_id = $1 WHERE id = ANY($2) AND person_id IS NULL")
                .bind(person)
                .bind(&ids)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
    }
    Ok(())
}

pub async fn ocr(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let asset = get_asset(&state.db, asset_id).await?;
    let res = predict(state, &asset, "ocr").await?;
    let text = res.ocr.map(|o| o.text).unwrap_or_default();
    if text.trim().is_empty() {
        sqlx::query("DELETE FROM ocr WHERE asset_id = $1").bind(asset_id).execute(&state.db).await?;
    } else {
        sqlx::query("INSERT INTO ocr (asset_id, text) VALUES ($1, $2) ON CONFLICT (asset_id) DO UPDATE SET text = $2")
            .bind(asset_id)
            .bind(text.trim())
            .execute(&state.db)
            .await?;
    }
    mark(state, asset_id, "ocr_at").await?;
    Ok(())
}

pub async fn objects(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let asset = get_asset(&state.db, asset_id).await?;
    let res = predict(state, &asset, "objects").await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM asset_objects WHERE asset_id = $1").bind(asset_id).execute(&mut *tx).await?;
    for o in res.objects.unwrap_or_default() {
        sqlx::query("INSERT INTO asset_objects (asset_id, label, score) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING")
            .bind(asset_id)
            .bind(o.label.to_lowercase())
            .bind(o.score)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE assets SET objects_at = now() WHERE id = $1").bind(asset_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Groups near-identical images (CLIP distance below threshold) under one duplicate id.
pub async fn duplicates(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let max = state.sys().ml.duplicate_max_distance as f64;
    let asset = get_asset(&state.db, asset_id).await?;
    let matches: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT a.id, a.duplicate_id FROM smart_search s JOIN assets a ON a.id = s.asset_id,
                (SELECT embedding FROM smart_search WHERE asset_id = $1) me
         WHERE a.owner_id = $2 AND a.id <> $1 AND a.deleted_at IS NULL AND a.visibility <> 'hidden'
           AND a.asset_type = $3 AND (s.embedding <=> me.embedding) <= $4
         ORDER BY s.embedding <=> me.embedding LIMIT 20",
    )
    .bind(asset_id)
    .bind(asset.owner_id)
    .bind(&asset.asset_type)
    .bind(max)
    .fetch_all(&state.db)
    .await?;
    if !matches.is_empty() {
        let group = asset
            .duplicate_id
            .or_else(|| matches.iter().find_map(|(_, d)| *d))
            .unwrap_or_else(Uuid::new_v4);
        let mut ids: Vec<Uuid> = matches.iter().map(|(id, _)| *id).collect();
        ids.push(asset_id);
        let old: Vec<Uuid> = matches.iter().filter_map(|(_, d)| *d).filter(|d| *d != group).collect();
        sqlx::query("UPDATE assets SET duplicate_id = $1 WHERE id = ANY($2) OR duplicate_id = ANY($3)")
            .bind(group)
            .bind(&ids)
            .bind(&old)
            .execute(&state.db)
            .await?;
    }
    mark(state, asset_id, "duplicates_at").await?;
    Ok(())
}
