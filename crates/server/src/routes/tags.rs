use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use pixelhort_common::IdsRequest;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::Auth;
use crate::error::{AppError, AppResult};
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/tags", get(list).post(create))
        .route("/tags/{id}", put(update).delete(remove))
        .route("/tags/{id}/assets", put(tag_assets).delete(untag_assets))
}

/// Creates `value` (e.g. `Holiday/Italy`) and all parents; returns the leaf id.
pub async fn upsert_tag(state: &SharedState, user: Uuid, value: &str, color: Option<&str>) -> anyhow::Result<Uuid> {
    let parts: Vec<&str> = value.split('/').map(str::trim).filter(|p| !p.is_empty()).collect();
    anyhow::ensure!(!parts.is_empty(), "empty tag");
    let mut parent: Option<Uuid> = None;
    let mut path = String::new();
    for (i, part) in parts.iter().enumerate() {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(part);
        let leaf_color = if i == parts.len() - 1 { color } else { None };
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO tags (id, user_id, value, color, parent_id) VALUES ($1,$2,$3,$4,$5)
             ON CONFLICT (user_id, value) DO UPDATE SET color = COALESCE(EXCLUDED.color, tags.color)
             RETURNING id",
        )
        .bind(Uuid::new_v4())
        .bind(user)
        .bind(&path)
        .bind(leaf_color)
        .bind(parent)
        .fetch_one(&state.db)
        .await?;
        parent = Some(id);
    }
    Ok(parent.expect("at least one part"))
}

async fn tag_json(state: &SharedState, user: Uuid, id: Option<Uuid>) -> AppResult<Vec<Value>> {
    let rows: Vec<(Uuid, String, Option<String>, Option<Uuid>, i64)> = sqlx::query_as(
        "SELECT t.id, t.value, t.color, t.parent_id,
            (SELECT COUNT(*) FROM tag_assets ta JOIN assets a ON a.id = ta.asset_id
              WHERE ta.tag_id = t.id AND a.deleted_at IS NULL)
         FROM tags t WHERE t.user_id = $1 AND ($2::uuid IS NULL OR t.id = $2) ORDER BY t.value",
    )
    .bind(user)
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, value, color, parent, count)| {
            let name = value.rsplit('/').next().unwrap_or(&value).to_string();
            json!({"id": id, "value": value, "name": name, "color": color, "parentId": parent, "assetCount": count})
        })
        .collect())
}

async fn list(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Vec<Value>>> {
    Ok(Json(tag_json(&state, auth.id(), None).await?))
}

#[derive(Deserialize)]
struct CreateTag {
    value: String,
    color: Option<String>,
}

async fn create(State(state): State<SharedState>, auth: Auth, Json(req): Json<CreateTag>) -> AppResult<Json<Value>> {
    let id = upsert_tag(&state, auth.id(), &req.value, req.color.as_deref())
        .await
        .map_err(|e| AppError::bad(e.to_string()))?;
    tag_json(&state, auth.id(), Some(id)).await?.pop().map(Json).ok_or_else(AppError::not_found)
}

#[derive(Deserialize)]
struct UpdateTag {
    color: Option<String>,
}

async fn update(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<UpdateTag>) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE tags SET color = $3 WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.id())
        .bind(req.color)
        .execute(&state.db)
        .await?;
    tag_json(&state, auth.id(), Some(id)).await?.pop().map(Json).ok_or_else(AppError::not_found)
}

async fn remove(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2").bind(id).bind(auth.id()).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn owned_tag(state: &SharedState, auth: &Auth, id: Uuid) -> AppResult<()> {
    let ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM tags WHERE id = $1 AND user_id = $2)")
        .bind(id)
        .bind(auth.id())
        .fetch_one(&state.db)
        .await?;
    if ok { Ok(()) } else { Err(AppError::not_found()) }
}

async fn tag_assets(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<IdsRequest>) -> AppResult<StatusCode> {
    owned_tag(&state, &auth, id).await?;
    sqlx::query(
        "INSERT INTO tag_assets (tag_id, asset_id) SELECT $1, a.id FROM assets a WHERE a.id = ANY($2) AND a.owner_id = $3
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(&req.ids)
    .bind(auth.id())
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE assets SET updated_at = now() WHERE id = ANY($1) AND owner_id = $2")
        .bind(&req.ids)
        .bind(auth.id())
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn untag_assets(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<IdsRequest>) -> AppResult<StatusCode> {
    owned_tag(&state, &auth, id).await?;
    sqlx::query("DELETE FROM tag_assets WHERE tag_id = $1 AND asset_id = ANY($2)")
        .bind(id)
        .bind(&req.ids)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
