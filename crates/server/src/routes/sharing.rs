//! Duplicates, external libraries, partner sharing and public shared links.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use pixelhort_common::{AssetDto, LibraryDto};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::access::{require_album_role, user_can_read};
use crate::auth::{AdminAuth, Auth, SharedLinkRow, hash_password, random_token, share_token, verify_password};
use crate::error::{AppError, AppResult};
use crate::jobs::library::{LibraryRow, build_globs};
use crate::model::{ASSET_COLUMNS, AssetRow, assets_to_dtos, get_asset, user_summary};
use crate::routes::albums::album_dto;
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/duplicates", get(list_duplicates))
        .route("/duplicates/{id}", delete(dismiss_duplicate))
        .route("/duplicates/{id}/resolve", post(resolve_duplicate))
        .route("/libraries", get(list_libraries).post(create_library))
        .route("/libraries/{id}", axum::routing::put(update_library).delete(delete_library))
        .route("/libraries/{id}/scan", post(scan_library))
        .route("/partners", get(list_partners))
        .route("/partners/{user_id}", post(add_partner).put(update_partner).delete(remove_partner))
        .route("/shared-links", get(list_links).post(create_link))
        .route("/shared-links/me", get(link_me))
        .route("/shared-links/me/login", post(link_login))
        .route("/shared-links/{id}", axum::routing::patch(update_link).delete(delete_link))
}

// ------------------------------------------------------------------ duplicates

async fn list_duplicates(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Value>> {
    let rows: Vec<AssetRow> = sqlx::query_as(&format!(
        "SELECT {ASSET_COLUMNS} FROM assets a WHERE a.owner_id = $1 AND a.duplicate_id IS NOT NULL
         AND a.deleted_at IS NULL AND a.visibility IN ('timeline','archive')
         ORDER BY a.duplicate_id, a.file_size DESC"
    ))
    .bind(auth.id())
    .fetch_all(&state.db)
    .await?;
    let mut groups: Vec<(Uuid, Vec<AssetDto>)> = Vec::new();
    for r in rows {
        let d = r.duplicate_id.expect("filtered");
        match groups.last_mut() {
            Some((g, list)) if *g == d => list.push(r.to_dto()),
            _ => groups.push((d, vec![r.to_dto()])),
        }
    }
    Ok(Json(json!(groups
        .into_iter()
        .filter(|(_, l)| l.len() > 1)
        .map(|(id, assets)| json!({"duplicateId": id, "assets": assets}))
        .collect::<Vec<_>>())))
}

async fn dismiss_duplicate(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    sqlx::query("UPDATE assets SET duplicate_id = NULL, duplicates_at = now() WHERE duplicate_id = $1 AND owner_id = $2")
        .bind(id)
        .bind(auth.id())
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Resolve {
    #[serde(default)]
    keep_asset_ids: Vec<Uuid>,
    #[serde(default)]
    trash_asset_ids: Vec<Uuid>,
}

async fn resolve_duplicate(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, Json(req): Json<Resolve>) -> AppResult<StatusCode> {
    sqlx::query(
        "UPDATE assets SET deleted_at = now(), duplicate_id = NULL, updated_at = now()
         WHERE id = ANY($1) AND owner_id = $2 AND duplicate_id = $3",
    )
    .bind(&req.trash_asset_ids)
    .bind(auth.id())
    .bind(id)
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE assets SET duplicate_id = NULL, duplicates_at = now() WHERE id = ANY($1) AND owner_id = $2")
        .bind(&req.keep_asset_ids)
        .bind(auth.id())
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ libraries

async fn library_dto(state: &SharedState, l: LibraryRow) -> AppResult<LibraryDto> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM assets WHERE library_id = $1")
        .bind(l.id)
        .fetch_one(&state.db)
        .await?;
    Ok(LibraryDto {
        id: l.id,
        owner_id: l.owner_id,
        name: l.name,
        import_paths: l.import_paths,
        exclusion_patterns: l.exclusion_patterns,
        watch: l.watch,
        asset_count: count,
        refreshed_at: l.refreshed_at,
        created_at: l.created_at,
    })
}

async fn list_libraries(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Vec<LibraryDto>>> {
    let rows: Vec<LibraryRow> = if auth.user.is_admin {
        sqlx::query_as("SELECT * FROM libraries ORDER BY name").fetch_all(&state.db).await?
    } else {
        sqlx::query_as("SELECT * FROM libraries WHERE owner_id = $1 ORDER BY name").bind(auth.id()).fetch_all(&state.db).await?
    };
    let mut out = Vec::new();
    for r in rows {
        out.push(library_dto(&state, r).await?);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBody {
    owner_id: Option<Uuid>,
    name: Option<String>,
    import_paths: Option<Vec<String>>,
    exclusion_patterns: Option<Vec<String>>,
    watch: Option<bool>,
}

fn validate_library(b: &LibraryBody) -> AppResult<()> {
    if let Some(paths) = &b.import_paths {
        for p in paths {
            if !std::path::Path::new(p).is_absolute() {
                return Err(AppError::bad(format!("import path must be absolute: {p}")));
            }
        }
    }
    if let Some(ex) = &b.exclusion_patterns {
        build_globs(ex).map_err(|e| AppError::bad(format!("invalid exclusion pattern: {e}")))?;
    }
    Ok(())
}

async fn create_library(State(state): State<SharedState>, AdminAuth(admin): AdminAuth, Json(b): Json<LibraryBody>) -> AppResult<Json<LibraryDto>> {
    validate_library(&b)?;
    let id = Uuid::new_v4();
    let row: LibraryRow = sqlx::query_as(
        "INSERT INTO libraries (id, owner_id, name, import_paths, exclusion_patterns, watch) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *",
    )
    .bind(id)
    .bind(b.owner_id.unwrap_or(admin.id()))
    .bind(b.name.unwrap_or_else(|| "Library".into()))
    .bind(b.import_paths.unwrap_or_default())
    .bind(b.exclusion_patterns.unwrap_or_else(|| vec!["**/@eaDir/**".into(), "**/.*/**".into(), "**/#recycle/**".into()]))
    .bind(b.watch.unwrap_or(false))
    .fetch_one(&state.db)
    .await?;
    state.jobs.enqueue("library", None, json!({"libraryId": id})).await?;
    Ok(Json(library_dto(&state, row).await?))
}

async fn update_library(State(state): State<SharedState>, _a: AdminAuth, Path(id): Path<Uuid>, Json(b): Json<LibraryBody>) -> AppResult<Json<LibraryDto>> {
    validate_library(&b)?;
    let row: LibraryRow = sqlx::query_as(
        "UPDATE libraries SET name = COALESCE($2, name), import_paths = COALESCE($3, import_paths),
            exclusion_patterns = COALESCE($4, exclusion_patterns), watch = COALESCE($5, watch), owner_id = COALESCE($6, owner_id)
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(b.name)
    .bind(b.import_paths)
    .bind(b.exclusion_patterns)
    .bind(b.watch)
    .bind(b.owner_id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(library_dto(&state, row).await?))
}

async fn delete_library(State(state): State<SharedState>, _a: AdminAuth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM assets WHERE library_id = $1").bind(id).fetch_all(&state.db).await?;
    crate::routes::assets::delete_permanently(&state, &ids).await?;
    sqlx::query("DELETE FROM libraries WHERE id = $1").bind(id).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn scan_library(State(state): State<SharedState>, _a: AdminAuth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    state.jobs.enqueue("library", None, json!({"libraryId": id})).await?;
    Ok(StatusCode::ACCEPTED)
}

// ------------------------------------------------------------------ partners

#[derive(Deserialize)]
struct PartnerQuery {
    direction: Option<String>,
}

async fn list_partners(State(state): State<SharedState>, auth: Auth, Query(q): Query<PartnerQuery>) -> AppResult<Json<Value>> {
    let sql = if q.direction.as_deref() == Some("shared-with") {
        "SELECT u.id, u.name, u.email, p.in_timeline FROM partners p JOIN users u ON u.id = p.shared_by_id WHERE p.shared_with_id = $1"
    } else {
        "SELECT u.id, u.name, u.email, p.in_timeline FROM partners p JOIN users u ON u.id = p.shared_with_id WHERE p.shared_by_id = $1"
    };
    let rows: Vec<(Uuid, String, String, bool)> = sqlx::query_as(sql).bind(auth.id()).fetch_all(&state.db).await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, name, email, t)| json!({"user": {"id": id, "name": name, "email": email}, "inTimeline": t}))
        .collect::<Vec<_>>())))
}

async fn add_partner(State(state): State<SharedState>, auth: Auth, Path(user): Path<Uuid>) -> AppResult<Json<Value>> {
    if user == auth.id() {
        return Err(AppError::bad("cannot partner with yourself"));
    }
    let summary = user_summary(&state.db, user).await?;
    sqlx::query("INSERT INTO partners (shared_by_id, shared_with_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
        .bind(auth.id())
        .bind(user)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"user": summary, "inTimeline": false})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartnerUpdate {
    in_timeline: bool,
}

async fn update_partner(State(state): State<SharedState>, auth: Auth, Path(user): Path<Uuid>, Json(b): Json<PartnerUpdate>) -> AppResult<Json<Value>> {
    let r = sqlx::query("UPDATE partners SET in_timeline = $3 WHERE shared_by_id = $1 AND shared_with_id = $2")
        .bind(user)
        .bind(auth.id())
        .bind(b.in_timeline)
        .execute(&state.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::not_found());
    }
    Ok(Json(json!({"user": user_summary(&state.db, user).await?, "inTimeline": b.in_timeline})))
}

async fn remove_partner(State(state): State<SharedState>, auth: Auth, Path(user): Path<Uuid>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM partners WHERE (shared_by_id = $1 AND shared_with_id = $2) OR (shared_by_id = $2 AND shared_with_id = $1)")
        .bind(auth.id())
        .bind(user)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ shared links

async fn link_dto(state: &SharedState, link: &SharedLinkRow, headers: &HeaderMap, with_album: bool) -> AppResult<Value> {
    let asset_ids: Vec<Uuid> = sqlx::query_scalar("SELECT asset_id FROM shared_link_assets WHERE shared_link_id = $1")
        .bind(link.id)
        .fetch_all(&state.db)
        .await?;
    let album = match (link.album_id, with_album) {
        (Some(a), true) => Some(serde_json::to_value(album_dto(state, a, false).await?).map_err(|e| AppError::Internal(e.into()))?),
        _ => None,
    };
    let base = crate::routes::assets::public_base(state, headers);
    Ok(json!({
        "id": link.id, "key": link.key, "type": link.link_type, "albumId": link.album_id, "album": album,
        "assetIds": asset_ids, "description": link.description, "hasPassword": link.password_hash.is_some(),
        "expiresAt": link.expires_at, "allowDownload": link.allow_download, "allowUpload": link.allow_upload,
        "showMetadata": link.show_metadata, "createdAt": link.created_at, "url": format!("{base}/s/{}", link.key),
    }))
}

async fn list_links(State(state): State<SharedState>, auth: Auth, headers: HeaderMap) -> AppResult<Json<Vec<Value>>> {
    let links: Vec<SharedLinkRow> = sqlx::query_as("SELECT * FROM shared_links WHERE user_id = $1 ORDER BY created_at DESC")
        .bind(auth.id())
        .fetch_all(&state.db)
        .await?;
    let mut out = Vec::new();
    for l in &links {
        out.push(link_dto(&state, l, &headers, true).await?);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkBody {
    #[serde(rename = "type")]
    link_type: Option<String>,
    album_id: Option<Uuid>,
    asset_ids: Option<Vec<Uuid>>,
    description: Option<String>,
    password: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    clear_expiry: bool,
    allow_download: Option<bool>,
    allow_upload: Option<bool>,
    show_metadata: Option<bool>,
}

async fn create_link(State(state): State<SharedState>, auth: Auth, headers: HeaderMap, Json(b): Json<LinkBody>) -> AppResult<Json<Value>> {
    let link_type = b.link_type.clone().unwrap_or_else(|| if b.album_id.is_some() { "ALBUM".into() } else { "INDIVIDUAL".into() });
    let id = Uuid::new_v4();
    match link_type.as_str() {
        "ALBUM" => {
            let album = b.album_id.ok_or_else(|| AppError::bad("albumId is required"))?;
            if require_album_role(&state, auth.id(), album, true).await? != "owner" {
                return Err(AppError::forbidden());
            }
        }
        "INDIVIDUAL" => {
            if b.asset_ids.as_ref().map(|a| a.is_empty()).unwrap_or(true) {
                return Err(AppError::bad("assetIds is required"));
            }
        }
        _ => return Err(AppError::bad("type must be ALBUM or INDIVIDUAL")),
    }
    let password = b.password.as_deref().filter(|p| !p.is_empty()).map(hash_password).transpose()?;
    let link: SharedLinkRow = sqlx::query_as(
        "INSERT INTO shared_links (id, user_id, key, link_type, album_id, description, password_hash, expires_at,
            allow_download, allow_upload, show_metadata)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING *",
    )
    .bind(id)
    .bind(auth.id())
    .bind(random_token(24))
    .bind(&link_type)
    .bind(if link_type == "ALBUM" { b.album_id } else { None })
    .bind(b.description.unwrap_or_default())
    .bind(password)
    .bind(b.expires_at)
    .bind(b.allow_download.unwrap_or(true))
    .bind(b.allow_upload.unwrap_or(false))
    .bind(b.show_metadata.unwrap_or(true))
    .fetch_one(&state.db)
    .await?;
    if link_type == "INDIVIDUAL" {
        for asset in b.asset_ids.unwrap_or_default() {
            let a = get_asset(&state.db, asset).await?;
            if a.visibility == "locked" || !user_can_read(&state, auth.id(), auth.unlocked, &a).await? {
                continue;
            }
            sqlx::query("INSERT INTO shared_link_assets (shared_link_id, asset_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
                .bind(id)
                .bind(asset)
                .execute(&state.db)
                .await?;
        }
    }
    Ok(Json(link_dto(&state, &link, &headers, true).await?))
}

async fn update_link(State(state): State<SharedState>, auth: Auth, headers: HeaderMap, Path(id): Path<Uuid>, Json(b): Json<LinkBody>) -> AppResult<Json<Value>> {
    let password_change = b.password.as_ref().map(|p| if p.is_empty() { Ok(None) } else { hash_password(p).map(Some) }).transpose()?;
    let link: SharedLinkRow = sqlx::query_as(
        "UPDATE shared_links SET description = COALESCE($3, description),
            password_hash = CASE WHEN $4 THEN $5 ELSE password_hash END,
            expires_at = CASE WHEN $7 THEN NULL ELSE COALESCE($6, expires_at) END,
            allow_download = COALESCE($8, allow_download), allow_upload = COALESCE($9, allow_upload),
            show_metadata = COALESCE($10, show_metadata)
         WHERE id = $1 AND user_id = $2 RETURNING *",
    )
    .bind(id)
    .bind(auth.id())
    .bind(b.description)
    .bind(password_change.is_some())
    .bind(password_change.flatten())
    .bind(b.expires_at)
    .bind(b.clear_expiry)
    .bind(b.allow_download)
    .bind(b.allow_upload)
    .bind(b.show_metadata)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(AppError::not_found)?;
    if let Some(ids) = b.asset_ids {
        sqlx::query("DELETE FROM shared_link_assets WHERE shared_link_id = $1").bind(id).execute(&state.db).await?;
        sqlx::query(
            "INSERT INTO shared_link_assets (shared_link_id, asset_id) SELECT $1, a.id FROM assets a
             WHERE a.id = ANY($2) AND a.owner_id = $3 AND a.visibility <> 'locked' ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(&ids)
        .bind(auth.id())
        .execute(&state.db)
        .await?;
    }
    Ok(Json(link_dto(&state, &link, &headers, true).await?))
}

async fn delete_link(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM shared_links WHERE id = $1 AND user_id = $2").bind(id).bind(auth.id()).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkMeQuery {
    key: String,
    share_token: Option<String>,
}

async fn find_link(state: &SharedState, key: &str) -> AppResult<SharedLinkRow> {
    let link: SharedLinkRow = sqlx::query_as("SELECT * FROM shared_links WHERE key = $1")
        .bind(key)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(AppError::not_found)?;
    if link.expires_at.map(|e| e < Utc::now()).unwrap_or(false) {
        return Err(AppError::Unauthorized("share link expired".into()));
    }
    Ok(link)
}

async fn link_me(State(state): State<SharedState>, headers: HeaderMap, Query(q): Query<LinkMeQuery>) -> AppResult<Json<Value>> {
    let link = find_link(&state, &q.key).await?;
    if link.password_hash.is_some() && q.share_token.as_deref() != Some(share_token(&state.secret, &link).as_str()) {
        return Err(AppError::Unauthorized("password_required".into()));
    }
    let rows: Vec<AssetRow> = sqlx::query_as(&format!(
        "SELECT {ASSET_COLUMNS} FROM assets a WHERE a.deleted_at IS NULL AND a.visibility IN ('timeline','archive') AND (
            a.id IN (SELECT asset_id FROM shared_link_assets WHERE shared_link_id = $1)
            OR a.id IN (SELECT asset_id FROM album_assets WHERE album_id = $2))
         ORDER BY a.local_date_time DESC"
    ))
    .bind(link.id)
    .bind(link.album_id)
    .fetch_all(&state.db)
    .await?;
    let mut dto = link_dto(&state, &link, &headers, true).await?;
    // Never expose the owner's e-mail address publicly.
    if let Some(album) = dto.get_mut("album").and_then(|a| a.as_object_mut()) {
        album.remove("albumUsers");
        if let Some(owner) = album.get_mut("owner").and_then(|o| o.as_object_mut()) {
            owner.remove("email");
        }
    }
    let mut assets = assets_to_dtos(&rows);
    if !link.show_metadata {
        for a in &mut assets {
            a.exif = None;
        }
    }
    Ok(Json(json!({ "link": dto, "assets": assets })))
}

#[derive(Deserialize)]
struct LinkLogin {
    key: String,
    password: String,
}

async fn link_login(State(state): State<SharedState>, Json(b): Json<LinkLogin>) -> AppResult<Json<Value>> {
    let link = find_link(&state, &b.key).await?;
    let ok = link.password_hash.as_deref().map(|h| verify_password(&b.password, h)).unwrap_or(true);
    if !ok {
        return Err(AppError::Unauthorized("wrong password".into()));
    }
    Ok(Json(json!({ "shareToken": share_token(&state.secret, &link) })))
}
