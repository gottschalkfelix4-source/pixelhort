use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use image::GenericImageView;
use pixelhort_common::{AssetDto, IdsRequest};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{AppError, AppResult};
use crate::model::{ASSET_COLUMNS, AssetRow, assets_to_dtos};
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/people", get(list))
        .route("/people/{id}", get(get_one).put(update))
        .route("/people/{id}/thumbnail", get(thumbnail))
        .route("/people/{id}/assets", get(assets))
        .route("/people/{id}/merge", post(merge))
        .route("/faces", get(faces))
        .route("/faces/{id}", put(reassign_face))
}

#[derive(sqlx::FromRow)]
struct PersonRow {
    id: Uuid,
    name: String,
    birth_date: Option<NaiveDate>,
    is_hidden: bool,
    updated_at: DateTime<Utc>,
    face_count: i64,
}

const PERSON_SELECT: &str = "SELECT p.id, p.name, p.birth_date, p.is_hidden, p.updated_at,
    (SELECT COUNT(DISTINCT f.asset_id) FROM faces f JOIN assets a ON a.id = f.asset_id
      WHERE f.person_id = p.id AND a.deleted_at IS NULL AND a.visibility IN ('timeline','archive')) AS face_count
    FROM people p";

fn person_json(p: &PersonRow) -> Value {
    json!({
        "id": p.id, "name": p.name, "birthDate": p.birth_date, "isHidden": p.is_hidden,
        "faceCount": p.face_count, "thumbnailUrl": format!("/api/people/{}/thumbnail", p.id), "updatedAt": p.updated_at,
    })
}

async fn person(state: &SharedState, owner: Uuid, id: Uuid) -> AppResult<PersonRow> {
    sqlx::query_as(&format!("{PERSON_SELECT} WHERE p.id = $1 AND p.owner_id = $2"))
        .bind(id)
        .bind(owner)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)
}

pub async fn person_value(state: &SharedState, owner: Uuid, id: Uuid) -> AppResult<Value> {
    Ok(person_json(&person(state, owner, id).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListQuery {
    with_hidden: Option<bool>,
}

pub async fn list_people(state: &SharedState, owner: Uuid, with_hidden: bool, limit: i64) -> AppResult<Vec<Value>> {
    let rows: Vec<PersonRow> = sqlx::query_as(&format!(
        "SELECT * FROM ({PERSON_SELECT} WHERE p.owner_id = $1 AND ($2 OR NOT p.is_hidden)) x
         WHERE face_count > 0 ORDER BY (name = '') ASC, face_count DESC, name LIMIT $3"
    ))
    .bind(owner)
    .bind(with_hidden)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows.iter().map(person_json).collect())
}

async fn list(State(state): State<SharedState>, auth: Auth, Query(q): Query<ListQuery>) -> AppResult<Json<Value>> {
    let people = list_people(&state, auth.id(), q.with_hidden.unwrap_or(false), 10_000).await?;
    let (total, hidden): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE is_hidden) FROM people p WHERE owner_id = $1
         AND EXISTS (SELECT 1 FROM faces f WHERE f.person_id = p.id)",
    )
    .bind(auth.id())
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({"people": people, "total": total, "hidden": hidden})))
}

async fn get_one(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    Ok(Json(person_value(&state, auth.id(), id).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdatePerson {
    name: Option<String>,
    birth_date: Option<NaiveDate>,
    is_hidden: Option<bool>,
    feature_face_id: Option<Uuid>,
}

async fn update(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<UpdatePerson>) -> AppResult<Json<Value>> {
    person(&state, auth.id(), id).await?;
    if let Some(face) = req.feature_face_id {
        let ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM faces WHERE id = $1 AND person_id = $2)")
            .bind(face)
            .bind(id)
            .fetch_one(&state.db)
            .await?;
        if !ok {
            return Err(AppError::bad("face does not belong to this person"));
        }
        let _ = tokio::fs::remove_file(state.storage.person_thumb_path(auth.id(), id)).await;
    }
    sqlx::query(
        "UPDATE people SET name = COALESCE($2, name), birth_date = COALESCE($3, birth_date),
            is_hidden = COALESCE($4, is_hidden), feature_face_id = COALESCE($5, feature_face_id), updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(req.name.map(|n| n.trim().to_string()))
    .bind(req.birth_date)
    .bind(req.is_hidden)
    .bind(req.feature_face_id)
    .execute(&state.db)
    .await?;
    Ok(Json(person_value(&state, auth.id(), id).await?))
}

async fn thumbnail(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<Response> {
    person(&state, auth.id(), id).await?;
    let path = state.storage.person_thumb_path(auth.id(), id);
    if !path.exists() {
        let face: Option<(String, i32, i32, i32, i32, i32, i32)> = sqlx::query_as(
            "SELECT a.preview_path, f.x1, f.y1, f.x2, f.y2, f.image_width, f.image_height
             FROM faces f JOIN assets a ON a.id = f.asset_id JOIN people p ON p.id = f.person_id
             WHERE f.person_id = $1 AND a.preview_path IS NOT NULL AND a.deleted_at IS NULL
             ORDER BY (f.id = p.feature_face_id) DESC, f.score DESC, (f.x2 - f.x1) DESC LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
        let (preview, x1, y1, x2, y2, iw, ih) = face.ok_or_else(AppError::not_found)?;
        let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
            let img = image::open(&preview)?;
            let (w, h) = img.dimensions();
            let sx = w as f32 / iw.max(1) as f32;
            let sy = h as f32 / ih.max(1) as f32;
            let (fx1, fy1, fx2, fy2) = (x1 as f32 * sx, y1 as f32 * sy, x2 as f32 * sx, y2 as f32 * sy);
            let size = (fx2 - fx1).max(fy2 - fy1) * 1.5;
            let cx = (fx1 + fx2) / 2.0;
            let cy = (fy1 + fy2) / 2.0;
            let left = (cx - size / 2.0).max(0.0) as u32;
            let top = (cy - size / 2.0).max(0.0) as u32;
            let side = (size as u32).min(w - left).min(h - top).max(1);
            let crop = img.crop_imm(left, top, side, side).resize(256, 256, image::imageops::FilterType::Lanczos3);
            crate::media::encode_jpeg(&crop, 85)
        })
        .await
        .map_err(|e| AppError::Internal(e.into()))??;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(&path, &bytes).await?;
    }
    let bytes = tokio::fs::read(&path).await?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg"), (header::CACHE_CONTROL, "private, max-age=3600")], bytes).into_response())
}

async fn assets(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<Json<Vec<AssetDto>>> {
    person(&state, auth.id(), id).await?;
    let rows: Vec<AssetRow> = sqlx::query_as(&format!(
        "SELECT {ASSET_COLUMNS} FROM assets a WHERE a.deleted_at IS NULL AND a.visibility IN ('timeline','archive')
         AND EXISTS (SELECT 1 FROM faces f WHERE f.asset_id = a.id AND f.person_id = $1) ORDER BY a.local_date_time DESC"
    ))
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(assets_to_dtos(&rows)))
}

async fn merge(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<IdsRequest>) -> AppResult<Json<Value>> {
    person(&state, auth.id(), id).await?;
    let others: Vec<Uuid> = req.ids.into_iter().filter(|o| *o != id).collect();
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE faces SET person_id = $1 WHERE person_id = ANY($2) AND owner_id = $3")
        .bind(id)
        .bind(&others)
        .bind(auth.id())
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM people WHERE id = ANY($1) AND owner_id = $2").bind(&others).bind(auth.id()).execute(&mut *tx).await?;
    tx.commit().await?;
    for o in &others {
        let _ = tokio::fs::remove_file(state.storage.person_thumb_path(auth.id(), *o)).await;
    }
    Ok(Json(person_value(&state, auth.id(), id).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FacesQuery {
    asset_id: Uuid,
}

#[derive(sqlx::FromRow)]
struct FaceRow {
    id: Uuid,
    person_id: Option<Uuid>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    image_width: i32,
    image_height: i32,
}

async fn face_json(state: &SharedState, owner: Uuid, f: &FaceRow) -> AppResult<Value> {
    let person = match f.person_id {
        Some(p) => Some(person_value(state, owner, p).await?),
        None => None,
    };
    Ok(json!({
        "id": f.id, "personId": f.person_id, "person": person,
        "boundingBoxX1": f.x1, "boundingBoxY1": f.y1, "boundingBoxX2": f.x2, "boundingBoxY2": f.y2,
        "imageWidth": f.image_width, "imageHeight": f.image_height,
    }))
}

async fn faces(State(state): State<SharedState>, auth: Auth, Query(q): Query<FacesQuery>) -> AppResult<Json<Vec<Value>>> {
    let rows: Vec<FaceRow> = sqlx::query_as(
        "SELECT id, person_id, x1, y1, x2, y2, image_width, image_height FROM faces WHERE asset_id = $1 AND owner_id = $2",
    )
    .bind(q.asset_id)
    .bind(auth.id())
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::new();
    for f in &rows {
        out.push(face_json(&state, auth.id(), f).await?);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReassignFace {
    person_id: Option<Uuid>,
    #[serde(default)]
    create_person: bool,
}

async fn reassign_face(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<ReassignFace>) -> AppResult<Json<Value>> {
    let target = match (req.person_id, req.create_person) {
        (Some(p), _) => {
            person(&state, auth.id(), p).await?;
            Some(p)
        }
        (None, true) => {
            let p = Uuid::new_v4();
            sqlx::query("INSERT INTO people (id, owner_id, feature_face_id) VALUES ($1,$2,$3)")
                .bind(p)
                .bind(auth.id())
                .bind(id)
                .execute(&state.db)
                .await?;
            Some(p)
        }
        (None, false) => None,
    };
    let row: FaceRow = sqlx::query_as(
        "UPDATE faces SET person_id = $3 WHERE id = $1 AND owner_id = $2
         RETURNING id, person_id, x1, y1, x2, y2, image_width, image_height",
    )
    .bind(id)
    .bind(auth.id())
    .bind(target)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(AppError::not_found)?;
    Ok(Json(face_json(&state, auth.id(), &row).await?))
}
