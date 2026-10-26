use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use pgvector::Vector;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{AppError, AppResult};
use crate::model::{ASSET_COLUMNS, AssetRow, assets_to_dtos};
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/search/smart", post(smart))
        .route("/search/metadata", post(metadata))
        .route("/search/suggestions", get(suggestions))
        .route("/search/explore", get(explore))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Filters {
    query: Option<String>,
    original_file_name: Option<String>,
    text: Option<String>,
    make: Option<String>,
    model: Option<String>,
    lens_model: Option<String>,
    city: Option<String>,
    state: Option<String>,
    country: Option<String>,
    taken_after: Option<DateTime<Utc>>,
    taken_before: Option<DateTime<Utc>>,
    #[serde(rename = "type")]
    asset_type: Option<String>,
    is_favorite: Option<bool>,
    #[serde(default)]
    person_ids: Vec<Uuid>,
    #[serde(default)]
    tag_ids: Vec<Uuid>,
    #[serde(default)]
    objects: Vec<String>,
    is_raw: Option<bool>,
    is_panorama: Option<bool>,
    #[serde(rename = "is360")]
    is_360: Option<bool>,
    is_motion: Option<bool>,
    visibility: Option<String>,
    page: Option<i64>,
    size: Option<i64>,
}

fn push_filters<'a>(auth: &Auth, f: &'a Filters, qb: &mut QueryBuilder<'a, Postgres>) -> AppResult<()> {
    qb.push(" WHERE a.owner_id = ").push_bind(auth.id()).push(" AND a.deleted_at IS NULL");
    match f.visibility.as_deref() {
        Some("locked") => {
            auth.require_unlocked()?;
            qb.push(" AND a.visibility = 'locked'");
        }
        Some("archive") => {
            qb.push(" AND a.visibility = 'archive'");
        }
        Some("timeline") => {
            qb.push(" AND a.visibility = 'timeline'");
        }
        _ => {
            qb.push(" AND a.visibility IN ('timeline', 'archive')");
        }
    }
    let ilike = |qb: &mut QueryBuilder<'a, Postgres>, col: &str, v: &'a Option<String>| {
        if let Some(v) = v.as_ref().filter(|v| !v.is_empty()) {
            qb.push(format!(" AND {col} ILIKE ")).push_bind(format!("%{v}%"));
        }
    };
    ilike(qb, "a.original_file_name", &f.original_file_name);
    ilike(qb, "e.make", &f.make);
    ilike(qb, "e.model", &f.model);
    ilike(qb, "e.lens_model", &f.lens_model);
    ilike(qb, "e.city", &f.city);
    ilike(qb, "e.state", &f.state);
    ilike(qb, "e.country", &f.country);
    if let Some(t) = f.text.as_ref().filter(|t| !t.trim().is_empty()) {
        qb.push(" AND EXISTS (SELECT 1 FROM ocr o WHERE o.asset_id = a.id AND (o.tsv @@ plainto_tsquery('simple', ")
            .push_bind(t.clone())
            .push(") OR o.text ILIKE ")
            .push_bind(format!("%{}%", t.trim()))
            .push("))");
    }
    if let Some(d) = f.taken_after {
        qb.push(" AND a.local_date_time >= ").push_bind(d);
    }
    if let Some(d) = f.taken_before {
        qb.push(" AND a.local_date_time <= ").push_bind(d);
    }
    if let Some(t) = &f.asset_type {
        qb.push(" AND a.asset_type = ").push_bind(t.to_uppercase());
    }
    if let Some(v) = f.is_favorite {
        qb.push(" AND a.is_favorite = ").push_bind(v);
    }
    if let Some(v) = f.is_raw {
        qb.push(" AND a.is_raw = ").push_bind(v);
    }
    if let Some(v) = f.is_panorama {
        qb.push(" AND a.is_panorama = ").push_bind(v);
    }
    if let Some(v) = f.is_360 {
        qb.push(if v { " AND a.projection_type IS NOT NULL" } else { " AND a.projection_type IS NULL" });
    }
    if let Some(v) = f.is_motion {
        qb.push(if v { " AND a.live_photo_video_id IS NOT NULL" } else { " AND a.live_photo_video_id IS NULL" });
    }
    for p in &f.person_ids {
        qb.push(" AND EXISTS (SELECT 1 FROM faces fa WHERE fa.asset_id = a.id AND fa.person_id = ").push_bind(*p).push(")");
    }
    for t in &f.tag_ids {
        qb.push(" AND EXISTS (SELECT 1 FROM tag_assets ta WHERE ta.asset_id = a.id AND ta.tag_id = ").push_bind(*t).push(")");
    }
    for o in &f.objects {
        qb.push(" AND EXISTS (SELECT 1 FROM asset_objects ao WHERE ao.asset_id = a.id AND ao.label = ")
            .push_bind(o.to_lowercase())
            .push(")");
    }
    Ok(())
}

fn paging(f: &Filters) -> (i64, i64) {
    let size = f.size.unwrap_or(100).clamp(1, 1000);
    let page = f.page.unwrap_or(1).max(1);
    (page, size)
}

fn response(rows: Vec<AssetRow>, page: i64, size: i64) -> Value {
    let has_more = rows.len() as i64 > size;
    let rows: Vec<AssetRow> = rows.into_iter().take(size as usize).collect();
    json!({
        "assets": assets_to_dtos(&rows),
        "nextPage": if has_more { Some(page + 1) } else { None },
        "total": (page - 1) * size + rows.len() as i64,
    })
}

async fn smart(State(state): State<SharedState>, auth: Auth, Json(f): Json<Filters>) -> AppResult<Json<Value>> {
    let query = f.query.clone().filter(|q| !q.trim().is_empty()).ok_or_else(|| AppError::bad("query is required"))?;
    let embedding = crate::jobs::ml::clip_text(&state, &query)
        .await
        .map_err(|e| AppError::Unavailable(format!("smart search unavailable: {e}")))?;
    let (page, size) = paging(&f);
    let mut qb = QueryBuilder::new(format!(
        "SELECT {ASSET_COLUMNS} FROM assets a JOIN smart_search s ON s.asset_id = a.id LEFT JOIN exif e ON e.asset_id = a.id"
    ));
    push_filters(&auth, &f, &mut qb)?;
    qb.push(" ORDER BY s.embedding <=> ").push_bind(Vector::from(embedding));
    qb.push(" LIMIT ").push_bind(size + 1).push(" OFFSET ").push_bind((page - 1) * size);
    let rows: Vec<AssetRow> = qb.build_query_as().fetch_all(&state.db).await?;
    metrics::counter!("pixelhort_searches_total", "kind" => "smart").increment(1);
    Ok(Json(response(rows, page, size)))
}

async fn metadata(State(state): State<SharedState>, auth: Auth, Json(f): Json<Filters>) -> AppResult<Json<Value>> {
    let (page, size) = paging(&f);
    let mut qb = QueryBuilder::new(format!("SELECT {ASSET_COLUMNS} FROM assets a LEFT JOIN exif e ON e.asset_id = a.id"));
    push_filters(&auth, &f, &mut qb)?;
    qb.push(" ORDER BY a.local_date_time DESC LIMIT ").push_bind(size + 1).push(" OFFSET ").push_bind((page - 1) * size);
    let rows: Vec<AssetRow> = qb.build_query_as().fetch_all(&state.db).await?;
    metrics::counter!("pixelhort_searches_total", "kind" => "metadata").increment(1);
    Ok(Json(response(rows, page, size)))
}

#[derive(Deserialize)]
struct SuggestionQuery {
    #[serde(rename = "type")]
    kind: String,
    q: Option<String>,
}

async fn suggestions(State(state): State<SharedState>, auth: Auth, Query(q): Query<SuggestionQuery>) -> AppResult<Json<Vec<String>>> {
    let (col, from) = match q.kind.as_str() {
        "city" => ("e.city", "exif e"),
        "state" => ("e.state", "exif e"),
        "country" => ("e.country", "exif e"),
        "make" => ("e.make", "exif e"),
        "model" => ("e.model", "exif e"),
        "lens" => ("e.lens_model", "exif e"),
        "object" => ("e.label", "asset_objects e"),
        _ => return Err(AppError::bad("unknown suggestion type")),
    };
    let sql = format!(
        "SELECT DISTINCT {col} FROM {from} JOIN assets a ON a.id = e.asset_id
         WHERE a.owner_id = $1 AND a.deleted_at IS NULL AND {col} IS NOT NULL AND {col} ILIKE $2
         ORDER BY {col} LIMIT 50"
    );
    let values: Vec<String> = sqlx::query_scalar(&sql)
        .bind(auth.id())
        .bind(format!("%{}%", q.q.unwrap_or_default()))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(values))
}

async fn explore(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Value>> {
    let places: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT city, id FROM (
            SELECT DISTINCT ON (e.city) e.city, a.id, a.local_date_time FROM exif e JOIN assets a ON a.id = e.asset_id
            WHERE a.owner_id = $1 AND a.deleted_at IS NULL AND a.visibility = 'timeline' AND e.city IS NOT NULL
            ORDER BY e.city, a.local_date_time DESC) x
         ORDER BY local_date_time DESC LIMIT 50",
    )
    .bind(auth.id())
    .fetch_all(&state.db)
    .await?;
    let objects: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT label, asset_id FROM (
            SELECT ao.label, ao.asset_id, COUNT(*) OVER (PARTITION BY ao.label) AS n,
                   ROW_NUMBER() OVER (PARTITION BY ao.label ORDER BY ao.score DESC) AS rn
            FROM asset_objects ao JOIN assets a ON a.id = ao.asset_id
            WHERE a.owner_id = $1 AND a.deleted_at IS NULL AND a.visibility = 'timeline') x
         WHERE rn = 1 ORDER BY n DESC LIMIT 50",
    )
    .bind(auth.id())
    .fetch_all(&state.db)
    .await?;
    let people = crate::routes::people::list_people(&state, auth.id(), false, 20).await?;
    Ok(Json(json!({
        "places": places.into_iter().map(|(v, id)| json!({"value": v, "assetId": id})).collect::<Vec<_>>(),
        "objects": objects.into_iter().map(|(v, id)| json!({"value": v, "assetId": id})).collect::<Vec<_>>(),
        "people": people,
    })))
}
