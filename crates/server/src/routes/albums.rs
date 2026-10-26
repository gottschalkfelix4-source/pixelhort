use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use pixelhort_common::{AlbumDto, AlbumUserDto, AlbumUserRequest, BulkIdResult, CreateAlbumRequest, IdsRequest, UserSummary};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::access::{require_album_role, user_can_read};
use crate::auth::Auth;
use crate::error::{AppError, AppResult};
use crate::model::{ASSET_COLUMNS, AssetRow, assets_to_dtos, get_asset, user_summary};
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/albums", get(list).post(create))
        .route("/albums/{id}", get(get_one).patch(update).delete(remove))
        .route("/albums/{id}/assets", put(add_assets).delete(remove_assets))
        .route("/albums/{id}/users", put(add_users))
        .route("/albums/{id}/users/{user_id}", delete(remove_user))
        .route("/albums/{id}/activities", get(list_activities).post(create_activity))
        .route("/activities/{id}", delete(delete_activity))
}

#[derive(sqlx::FromRow)]
struct AlbumRow {
    id: Uuid,
    owner_id: Uuid,
    name: String,
    description: String,
    thumbnail_asset_id: Option<Uuid>,
    sort_order: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    asset_count: i64,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    first_asset: Option<Uuid>,
}

const ALBUM_SELECT: &str = "SELECT al.id, al.owner_id, al.name, al.description, al.thumbnail_asset_id, al.sort_order,
    al.created_at, al.updated_at,
    (SELECT COUNT(*) FROM album_assets aa JOIN assets a ON a.id = aa.asset_id
        WHERE aa.album_id = al.id AND a.deleted_at IS NULL) AS asset_count,
    (SELECT MIN(a.local_date_time) FROM album_assets aa JOIN assets a ON a.id = aa.asset_id
        WHERE aa.album_id = al.id AND a.deleted_at IS NULL) AS start_date,
    (SELECT MAX(a.local_date_time) FROM album_assets aa JOIN assets a ON a.id = aa.asset_id
        WHERE aa.album_id = al.id AND a.deleted_at IS NULL) AS end_date,
    (SELECT aa.asset_id FROM album_assets aa JOIN assets a ON a.id = aa.asset_id
        WHERE aa.album_id = al.id AND a.deleted_at IS NULL ORDER BY a.local_date_time DESC LIMIT 1) AS first_asset
    FROM albums al";

async fn to_dto(state: &SharedState, row: AlbumRow, with_assets: bool) -> AppResult<AlbumDto> {
    let users: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT u.id, u.name, u.email, au.role FROM album_users au JOIN users u ON u.id = au.user_id WHERE au.album_id = $1 ORDER BY u.name",
    )
    .bind(row.id)
    .fetch_all(&state.db)
    .await?;
    let assets = if with_assets {
        let order = if row.sort_order == "asc" { "ASC" } else { "DESC" };
        let rows: Vec<AssetRow> = sqlx::query_as(&format!(
            "SELECT {ASSET_COLUMNS} FROM assets a JOIN album_assets aa ON aa.asset_id = a.id
             WHERE aa.album_id = $1 AND a.deleted_at IS NULL AND a.visibility IN ('timeline','archive')
             ORDER BY a.local_date_time {order}"
        ))
        .bind(row.id)
        .fetch_all(&state.db)
        .await?;
        Some(assets_to_dtos(&rows))
    } else {
        None
    };
    Ok(AlbumDto {
        id: row.id,
        owner_id: row.owner_id,
        owner: user_summary(&state.db, row.owner_id).await?,
        album_name: row.name,
        description: row.description,
        album_thumbnail_asset_id: row.thumbnail_asset_id.or(row.first_asset),
        asset_count: row.asset_count,
        start_date: row.start_date,
        end_date: row.end_date,
        created_at: row.created_at,
        updated_at: row.updated_at,
        shared: !users.is_empty(),
        order: row.sort_order,
        album_users: users
            .into_iter()
            .map(|(id, name, email, role)| AlbumUserDto { user: UserSummary { id, name, email }, role })
            .collect(),
        assets,
    })
}

pub async fn album_dto(state: &SharedState, id: Uuid, with_assets: bool) -> AppResult<AlbumDto> {
    let row: AlbumRow = sqlx::query_as(&format!("{ALBUM_SELECT} WHERE al.id = $1")).bind(id).fetch_one(&state.db).await?;
    to_dto(state, row, with_assets).await
}

#[derive(Deserialize)]
struct ListQuery {
    shared: Option<bool>,
}

async fn list(State(state): State<SharedState>, auth: Auth, Query(q): Query<ListQuery>) -> AppResult<Json<Vec<AlbumDto>>> {
    let me = auth.id();
    let filter = match q.shared {
        // shared albums: albums others shared with me, or my albums I shared
        Some(true) => "WHERE (EXISTS (SELECT 1 FROM album_users au WHERE au.album_id = al.id AND au.user_id = $1))
                       OR (al.owner_id = $1 AND EXISTS (SELECT 1 FROM album_users au WHERE au.album_id = al.id))",
        Some(false) => "WHERE al.owner_id = $1 AND NOT EXISTS (SELECT 1 FROM album_users au WHERE au.album_id = al.id)",
        None => "WHERE al.owner_id = $1 OR EXISTS (SELECT 1 FROM album_users au WHERE au.album_id = al.id AND au.user_id = $1)",
    };
    let rows: Vec<AlbumRow> = sqlx::query_as(&format!("{ALBUM_SELECT} {filter} ORDER BY al.updated_at DESC"))
        .bind(me)
        .fetch_all(&state.db)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(to_dto(&state, r, false).await?);
    }
    Ok(Json(out))
}

async fn insert_assets(state: &SharedState, auth: &Auth, album: Uuid, ids: &[Uuid]) -> AppResult<Vec<BulkIdResult>> {
    let mut results = Vec::new();
    for id in ids {
        let asset = match get_asset(&state.db, *id).await {
            Ok(a) => a,
            Err(_) => {
                results.push(BulkIdResult { id: *id, success: false, error: Some("not_found".into()) });
                continue;
            }
        };
        if asset.visibility == "locked" || !user_can_read(state, auth.id(), auth.unlocked, &asset).await? {
            results.push(BulkIdResult { id: *id, success: false, error: Some("no_permission".into()) });
            continue;
        }
        let r = sqlx::query("INSERT INTO album_assets (album_id, asset_id, added_by) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING")
            .bind(album)
            .bind(id)
            .bind(auth.id())
            .execute(&state.db)
            .await?;
        results.push(if r.rows_affected() == 1 {
            BulkIdResult { id: *id, success: true, error: None }
        } else {
            BulkIdResult { id: *id, success: false, error: Some("duplicate".into()) }
        });
    }
    sqlx::query("UPDATE albums SET updated_at = now() WHERE id = $1").bind(album).execute(&state.db).await?;
    Ok(results)
}

async fn set_users(state: &SharedState, owner: Uuid, album: Uuid, users: &[AlbumUserRequest]) -> AppResult<()> {
    for u in users {
        if u.user_id == owner {
            continue;
        }
        let role = if u.role == "viewer" { "viewer" } else { "editor" };
        sqlx::query(
            "INSERT INTO album_users (album_id, user_id, role) VALUES ($1,$2,$3)
             ON CONFLICT (album_id, user_id) DO UPDATE SET role = $3",
        )
        .bind(album)
        .bind(u.user_id)
        .bind(role)
        .execute(&state.db)
        .await
        .map_err(|_| AppError::bad("unknown user"))?;
    }
    Ok(())
}

async fn create(State(state): State<SharedState>, auth: Auth, Json(req): Json<CreateAlbumRequest>) -> AppResult<Json<AlbumDto>> {
    if req.album_name.trim().is_empty() {
        return Err(AppError::bad("albumName is required"));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO albums (id, owner_id, name, description) VALUES ($1,$2,$3,$4)")
        .bind(id)
        .bind(auth.id())
        .bind(req.album_name.trim())
        .bind(req.description.unwrap_or_default())
        .execute(&state.db)
        .await?;
    insert_assets(&state, &auth, id, &req.asset_ids).await?;
    set_users(&state, auth.id(), id, &req.album_users).await?;
    Ok(Json(album_dto(&state, id, false).await?))
}

async fn get_one(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<Json<AlbumDto>> {
    require_album_role(&state, auth.id(), id, false).await?;
    Ok(Json(album_dto(&state, id, true).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAlbum {
    album_name: Option<String>,
    description: Option<String>,
    album_thumbnail_asset_id: Option<Uuid>,
    order: Option<String>,
}

async fn update(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<UpdateAlbum>) -> AppResult<Json<AlbumDto>> {
    let role = require_album_role(&state, auth.id(), id, true).await?;
    if role != "owner" && (req.album_name.is_some() || req.order.is_some()) {
        return Err(AppError::forbidden());
    }
    let order = req.order.filter(|o| o == "asc" || o == "desc");
    sqlx::query(
        "UPDATE albums SET name = COALESCE($2, name), description = COALESCE($3, description),
            thumbnail_asset_id = COALESCE($4, thumbnail_asset_id), sort_order = COALESCE($5, sort_order), updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(req.album_name)
    .bind(req.description)
    .bind(req.album_thumbnail_asset_id)
    .bind(order)
    .execute(&state.db)
    .await?;
    Ok(Json(album_dto(&state, id, false).await?))
}

async fn remove(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    if require_album_role(&state, auth.id(), id, true).await? != "owner" {
        return Err(AppError::forbidden());
    }
    sqlx::query("DELETE FROM albums WHERE id = $1").bind(id).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_assets(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<IdsRequest>) -> AppResult<Json<Vec<BulkIdResult>>> {
    require_album_role(&state, auth.id(), id, true).await?;
    Ok(Json(insert_assets(&state, &auth, id, &req.ids).await?))
}

async fn remove_assets(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<IdsRequest>) -> AppResult<Json<Vec<BulkIdResult>>> {
    let role = require_album_role(&state, auth.id(), id, true).await?;
    let mut results = Vec::new();
    for asset in &req.ids {
        // Editors may only remove what they added (or own).
        let sql = if role == "owner" {
            "DELETE FROM album_assets WHERE album_id = $1 AND asset_id = $2"
        } else {
            "DELETE FROM album_assets aa USING assets a WHERE aa.album_id = $1 AND aa.asset_id = $2 AND a.id = aa.asset_id
             AND (aa.added_by = $3 OR a.owner_id = $3)"
        };
        let r = sqlx::query(sql).bind(id).bind(asset).bind(auth.id()).execute(&state.db).await?;
        results.push(BulkIdResult {
            id: *asset,
            success: r.rows_affected() > 0,
            error: (r.rows_affected() == 0).then(|| "no_permission".to_string()),
        });
    }
    Ok(Json(results))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsersBody {
    album_users: Vec<AlbumUserRequest>,
}

async fn add_users(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<UsersBody>) -> AppResult<Json<AlbumDto>> {
    if require_album_role(&state, auth.id(), id, true).await? != "owner" {
        return Err(AppError::forbidden());
    }
    set_users(&state, auth.id(), id, &req.album_users).await?;
    sqlx::query("UPDATE albums SET updated_at = now() WHERE id = $1").bind(id).execute(&state.db).await?;
    Ok(Json(album_dto(&state, id, false).await?))
}

async fn remove_user(State(state): State<SharedState>, auth: Auth, Path((id, user)): Path<(Uuid, String)>) -> AppResult<StatusCode> {
    let role = require_album_role(&state, auth.id(), id, false).await?;
    let target = if user == "me" { auth.id() } else { Uuid::parse_str(&user).map_err(|_| AppError::bad("invalid user id"))? };
    if role != "owner" && target != auth.id() {
        return Err(AppError::forbidden());
    }
    sqlx::query("DELETE FROM album_users WHERE album_id = $1 AND user_id = $2").bind(id).bind(target).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_activities(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    require_album_role(&state, auth.id(), id, false).await?;
    let rows: Vec<(Uuid, Uuid, String, String, String, Option<String>, Option<Uuid>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT ac.id, u.id, u.name, u.email, ac.kind, ac.comment, ac.asset_id, ac.created_at
         FROM activities ac JOIN users u ON u.id = ac.user_id WHERE ac.album_id = $1 ORDER BY ac.created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(aid, uid, name, email, kind, comment, asset, created)| json!({
            "id": aid, "user": {"id": uid, "name": name, "email": email}, "type": kind,
            "comment": comment, "assetId": asset, "createdAt": created
        }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivityBody {
    #[serde(rename = "type")]
    kind: String,
    comment: Option<String>,
    asset_id: Option<Uuid>,
}

async fn create_activity(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<ActivityBody>) -> AppResult<Json<Value>> {
    require_album_role(&state, auth.id(), id, false).await?;
    if req.kind != "like" && req.kind != "comment" {
        return Err(AppError::bad("type must be like or comment"));
    }
    if req.kind == "comment" && req.comment.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(AppError::bad("comment is empty"));
    }
    if req.kind == "like" {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM activities WHERE album_id = $1 AND user_id = $2 AND kind = 'like' AND asset_id IS NOT DISTINCT FROM $3",
        )
        .bind(id)
        .bind(auth.id())
        .bind(req.asset_id)
        .fetch_optional(&state.db)
        .await?;
        if let Some(e) = existing {
            return Ok(Json(json!({"id": e, "type": "like", "assetId": req.asset_id})));
        }
    }
    let aid = Uuid::new_v4();
    let created: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO activities (id, album_id, user_id, asset_id, kind, comment) VALUES ($1,$2,$3,$4,$5,$6) RETURNING created_at",
    )
    .bind(aid)
    .bind(id)
    .bind(auth.id())
    .bind(req.asset_id)
    .bind(&req.kind)
    .bind(&req.comment)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({
        "id": aid, "user": {"id": auth.id(), "name": auth.user.name, "email": auth.user.email},
        "type": req.kind, "comment": req.comment, "assetId": req.asset_id, "createdAt": created
    })))
}

async fn delete_activity(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    sqlx::query(
        "DELETE FROM activities ac USING albums al WHERE ac.id = $1 AND al.id = ac.album_id AND (ac.user_id = $2 OR al.owner_id = $2)",
    )
    .bind(id)
    .bind(auth.id())
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
