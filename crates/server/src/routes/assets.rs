use std::path::PathBuf;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, TimeZone, Utc};
use pixelhort_common::{
    AssetDto, BulkUploadCheckRequest, BulkUploadCheckResponse, BulkUploadCheckResult, UploadResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;
use tower_http::services::ServeFile;
use uuid::Uuid;

use crate::access::{require_owner, require_read};
use crate::auth::{AnyAuth, Auth, Principal, cast_token};
use crate::error::{AppError, AppResult};
use crate::jobs::thumbnails;
use crate::media::{self, Edits};
use crate::model::{ASSET_COLUMNS, AssetRow, assets_to_dtos, full_asset_dto, get_asset};
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/assets", post(upload).put(bulk_update).delete(delete_assets))
        .layer(DefaultBodyLimit::disable())
        .route("/assets/bulk-upload-check", post(bulk_upload_check))
        .route("/assets/exist", post(exist))
        .route("/assets/statistics", get(statistics))
        .route("/assets/random", get(random))
        .route("/assets/{id}", get(get_one).put(update_one))
        .route("/assets/{id}/original", get(original))
        .route("/assets/{id}/thumbnail", get(thumbnail))
        .route("/assets/{id}/video", get(video))
        .route("/assets/{id}/edits", get(get_edits).put(put_edits).delete(delete_edits))
        .route("/assets/{id}/cast-url", post(cast_url))
        .route("/trash", get(list_trash))
        .route("/trash/restore", post(restore_trash))
        .route("/trash/empty", post(empty_trash))
}

// ------------------------------------------------------------------ upload

#[derive(Default)]
struct UploadFields {
    device_asset_id: Option<String>,
    device_id: Option<String>,
    file_created_at: Option<DateTime<Utc>>,
    file_modified_at: Option<DateTime<Utc>>,
    is_favorite: bool,
    live_photo_video_id: Option<Uuid>,
    visibility: Option<String>,
}

fn parse_date(v: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(v.trim())
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| v.trim().parse::<i64>().ok().and_then(|ms| Utc.timestamp_millis_opt(ms).single()))
}

struct TempFile {
    path: PathBuf,
    keep: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn user_usage(state: &SharedState, user: Uuid) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT COALESCE(SUM(file_size),0)::BIGINT FROM assets WHERE owner_id = $1 AND library_id IS NULL")
        .bind(user)
        .fetch_one(&state.db)
        .await?)
}

async fn upload(State(state): State<SharedState>, AnyAuth(principal): AnyAuth, headers: HeaderMap, mut multipart: Multipart) -> AppResult<Response> {
    let (owner, quota, unlocked, share_album) = match &principal {
        Principal::User { user, unlocked, .. } => (user.id, user.quota_bytes, *unlocked, None),
        Principal::Share { link } if link.allow_upload => {
            let quota: Option<i64> = sqlx::query_scalar("SELECT quota_bytes FROM users WHERE id = $1")
                .bind(link.user_id)
                .fetch_one(&state.db)
                .await?;
            (link.user_id, quota, false, Some(link.clone()))
        }
        _ => return Err(AppError::forbidden()),
    };

    if let Some(sum) = headers.get("x-checksum").and_then(|v| v.to_str().ok()) {
        let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM assets WHERE owner_id = $1 AND checksum = $2 AND library_id IS NULL")
            .bind(owner)
            .bind(sum.trim().to_lowercase())
            .fetch_optional(&state.db)
            .await?;
        if let Some(id) = existing {
            return Ok((StatusCode::OK, Json(UploadResponse { id, status: "duplicate".into() })).into_response());
        }
    }

    let mut fields = UploadFields::default();
    let mut file: Option<(TempFile, String, String, i64)> = None; // tmp, name, sha1, size
    let mut sidecar: Option<TempFile> = None;
    while let Some(mut field) = multipart.next_field().await.map_err(|e| AppError::bad(e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "assetData" | "sidecarData" => {
                let file_name = field.file_name().unwrap_or("upload").to_string();
                let tmp = TempFile { path: state.storage.tmp_file(), keep: false };
                let mut out = tokio::fs::File::create(&tmp.path).await?;
                let mut hasher = Sha1::new();
                let mut size: i64 = 0;
                while let Some(chunk) = field.chunk().await.map_err(|e| AppError::bad(e.to_string()))? {
                    hasher.update(&chunk);
                    size += chunk.len() as i64;
                    out.write_all(&chunk).await?;
                }
                out.flush().await?;
                if name == "assetData" {
                    file = Some((tmp, file_name, hex::encode(hasher.finalize()), size));
                } else {
                    sidecar = Some(tmp);
                }
            }
            _ => {
                let value = field.text().await.map_err(|e| AppError::bad(e.to_string()))?;
                match name.as_str() {
                    "deviceAssetId" => fields.device_asset_id = Some(value),
                    "deviceId" => fields.device_id = Some(value),
                    "fileCreatedAt" => fields.file_created_at = parse_date(&value),
                    "fileModifiedAt" => fields.file_modified_at = parse_date(&value),
                    "isFavorite" => fields.is_favorite = value == "true",
                    "livePhotoVideoId" => fields.live_photo_video_id = Uuid::parse_str(value.trim()).ok(),
                    "visibility" => fields.visibility = Some(value),
                    _ => {}
                }
            }
        }
    }
    let (mut tmp, file_name, checksum, size) = file.ok_or_else(|| AppError::bad("assetData missing"))?;
    let asset_type = pixelhort_common::media_type_for(&file_name)
        .ok_or_else(|| AppError::bad(format!("unsupported file type: {file_name}")))?;

    let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM assets WHERE owner_id = $1 AND checksum = $2 AND library_id IS NULL")
        .bind(owner)
        .bind(&checksum)
        .fetch_optional(&state.db)
        .await?;
    if let Some(id) = existing {
        metrics::counter!("pixelhort_uploads_total", "result" => "duplicate").increment(1);
        return Ok((StatusCode::OK, Json(UploadResponse { id, status: "duplicate".into() })).into_response());
    }
    if let Some(q) = quota {
        if user_usage(&state, owner).await? + size > q {
            return Err(AppError::bad("storage quota exceeded"));
        }
    }
    let visibility = match fields.visibility.as_deref() {
        Some("archive") => "archive",
        Some("locked") if unlocked => "locked",
        Some("locked") => return Err(AppError::Forbidden("locked".into())),
        _ => "timeline",
    };

    let now = Utc::now();
    let created = fields.file_created_at.unwrap_or(now);
    let modified = fields.file_modified_at.unwrap_or(created);
    let id = Uuid::new_v4();
    let ext = std::path::Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_ascii_lowercase();
    let dest = state.storage.original_path(owner, created, id, &ext);
    tokio::fs::create_dir_all(dest.parent().expect("parent")).await?;
    tokio::fs::rename(&tmp.path, &dest).await?;
    tmp.keep = true;
    let sidecar_path = match sidecar.as_mut() {
        Some(s) => {
            let p = dest.with_extension(format!("{ext}.xmp"));
            tokio::fs::rename(&s.path, &p).await?;
            s.keep = true;
            Some(p.to_string_lossy().to_string())
        }
        None => None,
    };

    let inserted = sqlx::query(
        "INSERT INTO assets (id, owner_id, device_asset_id, device_id, asset_type, original_path, original_file_name,
            original_mime_type, sidecar_path, checksum, file_size, file_created_at, file_modified_at, local_date_time,
            is_favorite, visibility, is_raw)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$12,$14,$15,$16) ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(owner)
    .bind(&fields.device_asset_id)
    .bind(&fields.device_id)
    .bind(asset_type)
    .bind(dest.to_string_lossy().to_string())
    .bind(&file_name)
    .bind(media::mime_for(&dest))
    .bind(&sidecar_path)
    .bind(&checksum)
    .bind(size)
    .bind(created)
    .bind(modified)
    .bind(fields.is_favorite)
    .bind(visibility)
    .bind(pixelhort_common::is_raw(&file_name))
    .execute(&state.db)
    .await?;
    if inserted.rows_affected() == 0 {
        // Concurrent upload of the same file won the race.
        let _ = tokio::fs::remove_file(&dest).await;
        let id: Uuid = sqlx::query_scalar("SELECT id FROM assets WHERE owner_id = $1 AND checksum = $2 AND library_id IS NULL")
            .bind(owner)
            .bind(&checksum)
            .fetch_one(&state.db)
            .await?;
        return Ok((StatusCode::OK, Json(UploadResponse { id, status: "duplicate".into() })).into_response());
    }

    if let Some(video) = fields.live_photo_video_id {
        let ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM assets WHERE id = $1 AND owner_id = $2 AND asset_type = 'VIDEO')")
            .bind(video)
            .bind(owner)
            .fetch_one(&state.db)
            .await?;
        if ok {
            crate::jobs::metadata::link_pair(&state, id, video).await?;
        }
    }
    if let Some(link) = share_album {
        if let Some(album) = link.album_id {
            sqlx::query("INSERT INTO album_assets (album_id, asset_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
                .bind(album)
                .bind(id)
                .execute(&state.db)
                .await?;
        } else {
            sqlx::query("INSERT INTO shared_link_assets (shared_link_id, asset_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
                .bind(link.id)
                .bind(id)
                .execute(&state.db)
                .await?;
        }
    }
    state.jobs.enqueue_asset("metadata", id).await?;
    metrics::counter!("pixelhort_uploads_total", "result" => "created").increment(1);
    metrics::counter!("pixelhort_upload_bytes_total").increment(size as u64);
    Ok((StatusCode::CREATED, Json(UploadResponse { id, status: "created".into() })).into_response())
}

async fn bulk_upload_check(
    State(state): State<SharedState>,
    auth: Auth,
    Json(req): Json<BulkUploadCheckRequest>,
) -> AppResult<Json<BulkUploadCheckResponse>> {
    let sums: Vec<String> = req.assets.iter().map(|a| a.checksum.trim().to_lowercase()).collect();
    let found: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, checksum FROM assets WHERE owner_id = $1 AND library_id IS NULL AND checksum = ANY($2)",
    )
    .bind(auth.id())
    .bind(&sums)
    .fetch_all(&state.db)
    .await?;
    let results = req
        .assets
        .into_iter()
        .map(|a| {
            let sum = a.checksum.trim().to_lowercase();
            match found.iter().find(|(_, c)| *c == sum) {
                Some((id, _)) => BulkUploadCheckResult {
                    id: a.id,
                    action: "reject".into(),
                    reason: Some("duplicate".into()),
                    asset_id: Some(*id),
                },
                None => BulkUploadCheckResult { id: a.id, action: "accept".into(), reason: None, asset_id: None },
            }
        })
        .collect();
    Ok(Json(BulkUploadCheckResponse { results }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExistRequest {
    device_id: String,
    device_asset_ids: Vec<String>,
}

async fn exist(State(state): State<SharedState>, auth: Auth, Json(req): Json<ExistRequest>) -> AppResult<Json<Value>> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT device_asset_id FROM assets WHERE owner_id = $1 AND device_id = $2 AND device_asset_id = ANY($3)",
    )
    .bind(auth.id())
    .bind(&req.device_id)
    .bind(&req.device_asset_ids)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({ "existingIds": ids })))
}

// ------------------------------------------------------------------ read/update

async fn get_one(State(state): State<SharedState>, AnyAuth(p): AnyAuth, Path(id): Path<Uuid>) -> AppResult<Json<AssetDto>> {
    let asset = get_asset(&state.db, id).await?;
    require_read(&state, &p, &asset).await?;
    let show_meta = match &p {
        Principal::Share { link } => link.show_metadata,
        _ => true,
    };
    Ok(Json(full_asset_dto(&state.db, &asset, show_meta).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAsset {
    is_favorite: Option<bool>,
    visibility: Option<String>,
    description: Option<String>,
    rating: Option<i16>,
    date_time_original: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

const VISIBILITIES: &[&str] = &["timeline", "archive", "hidden", "locked"];

async fn apply_visibility(state: &SharedState, auth: &Auth, ids: &[Uuid], visibility: &str) -> AppResult<()> {
    if !VISIBILITIES.contains(&visibility) || visibility == "hidden" {
        return Err(AppError::bad("invalid visibility"));
    }
    let touches_locked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM assets WHERE id = ANY($1) AND visibility = 'locked')")
        .bind(ids)
        .fetch_one(&state.db)
        .await?;
    if visibility == "locked" || touches_locked {
        auth.require_unlocked()?;
    }
    sqlx::query("UPDATE assets SET visibility = $3, updated_at = now() WHERE id = ANY($1) AND owner_id = $2 AND visibility <> 'hidden'")
        .bind(ids)
        .bind(auth.id())
        .bind(visibility)
        .execute(&state.db)
        .await?;
    if visibility == "locked" {
        // Locked assets leave albums and shares.
        sqlx::query("DELETE FROM album_assets WHERE asset_id = ANY($1)").bind(ids).execute(&state.db).await?;
        sqlx::query("DELETE FROM shared_link_assets WHERE asset_id = ANY($1)").bind(ids).execute(&state.db).await?;
    }
    Ok(())
}

async fn update_one(
    State(state): State<SharedState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAsset>,
) -> AppResult<Json<AssetDto>> {
    let asset = get_asset(&state.db, id).await?;
    require_owner(&auth, &asset)?;
    if let Some(v) = &req.visibility {
        apply_visibility(&state, &auth, &[id], v).await?;
    }
    if let Some(r) = req.rating {
        if !(0..=5).contains(&r) {
            return Err(AppError::bad("rating must be 0-5"));
        }
    }
    sqlx::query(
        "UPDATE assets SET is_favorite = COALESCE($2, is_favorite), description = COALESCE($3, description),
            rating = COALESCE($4, rating), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(req.is_favorite)
    .bind(&req.description)
    .bind(req.rating)
    .execute(&state.db)
    .await?;
    if let Some(d) = &req.date_time_original {
        let parsed = DateTime::parse_from_rfc3339(d).map_err(|_| AppError::bad("dateTimeOriginal must be RFC 3339"))?;
        let local = Utc.from_utc_datetime(&parsed.naive_local());
        sqlx::query("UPDATE assets SET local_date_time = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(local)
            .execute(&state.db)
            .await?;
        sqlx::query(
            "INSERT INTO exif (asset_id, date_time_original, time_zone) VALUES ($1,$2,$3)
             ON CONFLICT (asset_id) DO UPDATE SET date_time_original = $2, time_zone = $3",
        )
        .bind(id)
        .bind(parsed.with_timezone(&Utc))
        .bind(parsed.offset().to_string())
        .execute(&state.db)
        .await?;
    }
    if let (Some(lat), Some(lon)) = (req.latitude, req.longitude) {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return Err(AppError::bad("invalid coordinates"));
        }
        let place = state.geocoder.lookup(lat, lon);
        sqlx::query(
            "INSERT INTO exif (asset_id, latitude, longitude, city, state, country) VALUES ($1,$2,$3,$4,$5,$6)
             ON CONFLICT (asset_id) DO UPDATE SET latitude = $2, longitude = $3, city = $4, state = $5, country = $6",
        )
        .bind(id)
        .bind(lat)
        .bind(lon)
        .bind(place.as_ref().map(|p| p.city.clone()))
        .bind(place.as_ref().and_then(|p| p.state.clone()))
        .bind(place.as_ref().and_then(|p| p.country.clone()))
        .execute(&state.db)
        .await?;
        sqlx::query("UPDATE assets SET updated_at = now() WHERE id = $1").bind(id).execute(&state.db).await?;
    }
    let asset = get_asset(&state.db, id).await?;
    Ok(Json(full_asset_dto(&state.db, &asset, true).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUpdate {
    ids: Vec<Uuid>,
    is_favorite: Option<bool>,
    visibility: Option<String>,
    rating: Option<i16>,
}

async fn bulk_update(State(state): State<SharedState>, auth: Auth, Json(req): Json<BulkUpdate>) -> AppResult<StatusCode> {
    if let Some(v) = &req.visibility {
        apply_visibility(&state, &auth, &req.ids, v).await?;
    }
    let locked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM assets WHERE id = ANY($1) AND visibility = 'locked')")
        .bind(&req.ids)
        .fetch_one(&state.db)
        .await?;
    if locked {
        auth.require_unlocked()?;
    }
    sqlx::query(
        "UPDATE assets SET is_favorite = COALESCE($3, is_favorite), rating = COALESCE($4, rating), updated_at = now()
         WHERE id = ANY($1) AND owner_id = $2",
    )
    .bind(&req.ids)
    .bind(auth.id())
    .bind(req.is_favorite)
    .bind(req.rating)
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ delete / trash

#[derive(Deserialize)]
struct DeleteRequest {
    ids: Vec<Uuid>,
    #[serde(default)]
    force: bool,
}

/// Owned ids plus the hidden video parts of live/motion photos.
async fn with_live_parts(state: &SharedState, owner: Uuid, ids: &[Uuid]) -> AppResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM assets WHERE owner_id = $1 AND id = ANY($2)
         UNION SELECT live_photo_video_id FROM assets WHERE owner_id = $1 AND id = ANY($2) AND live_photo_video_id IS NOT NULL",
    )
    .bind(owner)
    .bind(ids)
    .fetch_all(&state.db)
    .await?)
}

async fn delete_assets(State(state): State<SharedState>, auth: Auth, Json(req): Json<DeleteRequest>) -> AppResult<StatusCode> {
    let ids = with_live_parts(&state, auth.id(), &req.ids).await?;
    let locked: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM assets WHERE id = ANY($1) AND visibility = 'locked')")
        .bind(&ids)
        .fetch_one(&state.db)
        .await?;
    if locked {
        auth.require_unlocked()?;
    }
    if req.force {
        delete_permanently(&state, &ids).await?;
    } else {
        sqlx::query("UPDATE assets SET deleted_at = now(), updated_at = now() WHERE id = ANY($1) AND deleted_at IS NULL")
            .bind(&ids)
            .execute(&state.db)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Removes assets from the database and schedules deletion of their files.
pub async fn delete_permanently(state: &SharedState, ids: &[Uuid]) -> anyhow::Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let mut all: Vec<Uuid> = ids.to_vec();
    let parts: Vec<Uuid> = sqlx::query_scalar("SELECT live_photo_video_id FROM assets WHERE id = ANY($1) AND live_photo_video_id IS NOT NULL")
        .bind(ids)
        .fetch_all(&state.db)
        .await?;
    all.extend(parts);
    let rows: Vec<AssetRow> = sqlx::query_as(&format!("SELECT {ASSET_COLUMNS} FROM assets a WHERE a.id = ANY($1)"))
        .bind(&all)
        .fetch_all(&state.db)
        .await?;
    let mut files = Vec::new();
    for a in &rows {
        if a.library_id.is_none() && state.storage.is_managed(std::path::Path::new(&a.original_path)) {
            files.push(a.original_path.clone());
            if let Some(s) = &a.sidecar_path {
                files.push(s.clone());
            }
        }
        files.extend(a.thumbnail_path.clone());
        files.extend(a.preview_path.clone());
        files.extend(a.encoded_video_path.clone());
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT INTO asset_deletions (asset_id, owner_id) SELECT id, owner_id FROM assets WHERE id = ANY($1)")
        .bind(&all)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE assets SET live_photo_video_id = NULL WHERE live_photo_video_id = ANY($1)")
        .bind(&all)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM assets WHERE id = ANY($1)").bind(&all).execute(&mut *tx).await?;
    tx.commit().await?;
    if !files.is_empty() {
        state.jobs.enqueue("cleanup", None, json!({ "kind": "files", "files": files })).await?;
    }
    Ok(())
}

async fn list_trash(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Vec<AssetDto>>> {
    let rows: Vec<AssetRow> = sqlx::query_as(&format!(
        "SELECT {ASSET_COLUMNS} FROM assets a WHERE a.owner_id = $1 AND a.deleted_at IS NOT NULL
         AND a.visibility <> 'hidden' AND (a.visibility <> 'locked' OR $2) ORDER BY a.deleted_at DESC"
    ))
    .bind(auth.id())
    .bind(auth.unlocked)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(assets_to_dtos(&rows)))
}

#[derive(Deserialize, Default)]
struct OptionalIds {
    ids: Option<Vec<Uuid>>,
}

async fn restore_trash(State(state): State<SharedState>, auth: Auth, body: Option<Json<OptionalIds>>) -> AppResult<StatusCode> {
    match body.and_then(|b| b.0.ids) {
        Some(ids) => {
            let ids = with_live_parts(&state, auth.id(), &ids).await?;
            sqlx::query("UPDATE assets SET deleted_at = NULL, updated_at = now() WHERE id = ANY($1)")
                .bind(&ids)
                .execute(&state.db)
                .await?;
        }
        None => {
            sqlx::query("UPDATE assets SET deleted_at = NULL, updated_at = now() WHERE owner_id = $1 AND deleted_at IS NOT NULL")
                .bind(auth.id())
                .execute(&state.db)
                .await?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn empty_trash(State(state): State<SharedState>, auth: Auth) -> AppResult<StatusCode> {
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM assets WHERE owner_id = $1 AND deleted_at IS NOT NULL")
        .bind(auth.id())
        .fetch_all(&state.db)
        .await?;
    delete_permanently(&state, &ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn statistics(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Value>> {
    let (images, videos): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE asset_type = 'IMAGE'), COUNT(*) FILTER (WHERE asset_type = 'VIDEO')
         FROM assets WHERE owner_id = $1 AND deleted_at IS NULL AND visibility IN ('timeline', 'archive')",
    )
    .bind(auth.id())
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({"images": images, "videos": videos, "total": images + videos})))
}

#[derive(Deserialize)]
struct RandomQuery {
    count: Option<i64>,
}

async fn random(State(state): State<SharedState>, auth: Auth, Query(q): Query<RandomQuery>) -> AppResult<Json<Vec<AssetDto>>> {
    let rows: Vec<AssetRow> = sqlx::query_as(&format!(
        "SELECT {ASSET_COLUMNS} FROM assets a WHERE a.owner_id = $1 AND a.deleted_at IS NULL AND a.visibility = 'timeline'
         ORDER BY random() LIMIT $2"
    ))
    .bind(auth.id())
    .bind(q.count.unwrap_or(10).clamp(1, 100))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(assets_to_dtos(&rows)))
}

// ------------------------------------------------------------------ media

async fn serve(path: &str, req: Request, content_type: Option<&str>, disposition: Option<String>) -> AppResult<Response> {
    if !std::path::Path::new(path).exists() {
        return Err(AppError::not_found());
    }
    let mut res = ServeFile::new(path)
        .oneshot(req)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("serve file: {e}")))?
        .map(Body::new);
    let h = res.headers_mut();
    if let Some(ct) = content_type.and_then(|c| HeaderValue::from_str(c).ok()) {
        h.insert(header::CONTENT_TYPE, ct);
    }
    if let Some(d) = disposition.and_then(|d| HeaderValue::from_str(&d).ok()) {
        h.insert(header::CONTENT_DISPOSITION, d);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400"));
    Ok(res)
}

fn attachment(name: &str) -> String {
    let ascii: String = name.chars().map(|c| if c.is_ascii_graphic() && c != '"' { c } else { '_' }).collect();
    format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{}", urlencoding::encode(name))
}

#[derive(Deserialize)]
struct OriginalQuery {
    edited: Option<bool>,
}

async fn original(
    State(state): State<SharedState>,
    AnyAuth(p): AnyAuth,
    Path(id): Path<Uuid>,
    Query(q): Query<OriginalQuery>,
    req: Request,
) -> AppResult<Response> {
    let asset = get_asset(&state.db, id).await?;
    require_read(&state, &p, &asset).await?;
    if let Principal::Share { link } = &p {
        if !link.allow_download {
            return Err(AppError::forbidden());
        }
    }
    if q.edited.unwrap_or(false) && thumbnails::edits_of(&asset).is_some() {
        let img = thumbnails::render(&state, &asset, true).await?;
        let bytes = tokio::task::spawn_blocking(move || media::encode_jpeg(&img, 92)).await.map_err(|e| AppError::Internal(e.into()))??;
        let stem = asset.original_file_name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&asset.original_file_name);
        return Ok((
            [
                (header::CONTENT_TYPE, "image/jpeg".to_string()),
                (header::CONTENT_DISPOSITION, attachment(&format!("{stem}_edited.jpg"))),
            ],
            bytes,
        )
            .into_response());
    }
    serve(&asset.original_path, req, asset.original_mime_type.as_deref(), Some(attachment(&asset.original_file_name))).await
}

#[derive(Deserialize)]
struct ThumbQuery {
    size: Option<String>,
}

async fn thumbnail(
    State(state): State<SharedState>,
    AnyAuth(p): AnyAuth,
    Path(id): Path<Uuid>,
    Query(q): Query<ThumbQuery>,
    req: Request,
) -> AppResult<Response> {
    let asset = get_asset(&state.db, id).await?;
    require_read(&state, &p, &asset).await?;
    let path = match q.size.as_deref() {
        Some("preview") | Some("fullsize") => asset.preview_path.or(asset.thumbnail_path),
        _ => asset.thumbnail_path.or(asset.preview_path),
    };
    let path = path.ok_or_else(AppError::not_found)?;
    serve(&path, req, Some("image/jpeg"), None).await
}

async fn video(State(state): State<SharedState>, AnyAuth(p): AnyAuth, Path(id): Path<Uuid>, req: Request) -> AppResult<Response> {
    let asset = get_asset(&state.db, id).await?;
    require_read(&state, &p, &asset).await?;
    if !asset.is_video() {
        return Err(AppError::bad("asset is not a video"));
    }
    match &asset.encoded_video_path {
        Some(encoded) => serve(encoded, req, Some("video/mp4"), None).await,
        None => {
            let ct = match asset.original_mime_type.as_deref() {
                Some("video/quicktime") => "video/mp4".to_string(), // H.264 MOV plays as MP4
                Some(m) => m.to_string(),
                None => "video/mp4".to_string(),
            };
            serve(&asset.original_path, req, Some(&ct), None).await
        }
    }
}

// ------------------------------------------------------------------ editor

async fn get_edits(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let asset = get_asset(&state.db, id).await?;
    require_owner(&auth, &asset)?;
    Ok(Json(asset.edits.unwrap_or(Value::Null)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EditRequest {
    #[serde(flatten)]
    edits: Edits,
    #[serde(default)]
    save_as_copy: bool,
}

async fn put_edits(
    State(state): State<SharedState>,
    auth: Auth,
    Path(id): Path<Uuid>,
    Json(req): Json<EditRequest>,
) -> AppResult<Json<AssetDto>> {
    let asset = get_asset(&state.db, id).await?;
    require_owner(&auth, &asset)?;
    if asset.is_video() {
        return Err(AppError::bad("videos cannot be edited"));
    }
    req.edits.validate().map_err(AppError::BadRequest)?;
    if req.save_as_copy {
        let mut tmp = asset.clone();
        tmp.edits = Some(serde_json::to_value(&req.edits).map_err(|e| AppError::Internal(e.into()))?);
        let img = thumbnails::render(&state, &tmp, true).await?;
        let bytes = tokio::task::spawn_blocking(move || media::encode_jpeg(&img, 92)).await.map_err(|e| AppError::Internal(e.into()))??;
        let new_id = Uuid::new_v4();
        let dest = state.storage.original_path(asset.owner_id, asset.file_created_at, new_id, "jpg");
        tokio::fs::create_dir_all(dest.parent().expect("parent")).await?;
        tokio::fs::write(&dest, &bytes).await?;
        let checksum = hex::encode(Sha1::digest(&bytes));
        let stem = asset.original_file_name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&asset.original_file_name);
        sqlx::query(
            "INSERT INTO assets (id, owner_id, asset_type, original_path, original_file_name, original_mime_type, checksum,
                file_size, file_created_at, file_modified_at, local_date_time, visibility, metadata_at, description, rating)
             VALUES ($1,$2,'IMAGE',$3,$4,'image/jpeg',$5,$6,$7,now(),$8,$9,now(),$10,$11)",
        )
        .bind(new_id)
        .bind(asset.owner_id)
        .bind(dest.to_string_lossy().to_string())
        .bind(format!("{stem}_edited.jpg"))
        .bind(checksum)
        .bind(bytes.len() as i64)
        .bind(asset.file_created_at)
        .bind(asset.local_date_time)
        .bind(&asset.visibility)
        .bind(&asset.description)
        .bind(asset.rating)
        .execute(&state.db)
        .await?;
        sqlx::query(
            "INSERT INTO exif (asset_id, make, model, lens_model, f_number, exposure_time, iso, focal_length, latitude,
                longitude, altitude, city, state, country, orientation, date_time_original, time_zone, description)
             SELECT $2, make, model, lens_model, f_number, exposure_time, iso, focal_length, latitude, longitude, altitude,
                city, state, country, 1, date_time_original, time_zone, description FROM exif WHERE asset_id = $1",
        )
        .bind(asset.id)
        .bind(new_id)
        .execute(&state.db)
        .await?;
        state.jobs.enqueue_asset("thumbnails", new_id).await?;
        let created = get_asset(&state.db, new_id).await?;
        return Ok(Json(full_asset_dto(&state.db, &created, true).await?));
    }
    let edits = if req.edits.is_identity() { None } else { Some(serde_json::to_value(&req.edits).map_err(|e| AppError::Internal(e.into()))?) };
    sqlx::query("UPDATE assets SET edits = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(edits)
        .execute(&state.db)
        .await?;
    // Regenerate synchronously so clients see the result immediately.
    if let Err(e) = thumbnails::run(&state, id).await {
        tracing::warn!("thumbnail regeneration failed: {e:#}");
        state.jobs.enqueue_asset("thumbnails", id).await?;
    }
    let asset = get_asset(&state.db, id).await?;
    Ok(Json(full_asset_dto(&state.db, &asset, true).await?))
}

async fn delete_edits(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<Json<AssetDto>> {
    let asset = get_asset(&state.db, id).await?;
    require_owner(&auth, &asset)?;
    sqlx::query("UPDATE assets SET edits = NULL, width = NULL, height = NULL, updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if let Err(e) = thumbnails::run(&state, id).await {
        tracing::warn!("thumbnail regeneration failed: {e:#}");
    }
    state.jobs.enqueue_asset("metadata", id).await?;
    let asset = get_asset(&state.db, id).await?;
    Ok(Json(full_asset_dto(&state.db, &asset, true).await?))
}

// ------------------------------------------------------------------ cast

pub fn public_base(state: &SharedState, headers: &HeaderMap) -> String {
    if let Some(u) = &state.config.public_url {
        return u.clone();
    }
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:2342");
    let proto = headers.get("x-forwarded-proto").and_then(|h| h.to_str().ok()).unwrap_or("http");
    format!("{proto}://{host}")
}

async fn cast_url(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let asset = get_asset(&state.db, id).await?;
    require_read(&state, &Principal::User { user: auth.user.clone(), session_id: auth.session_id, unlocked: auth.unlocked }, &asset).await?;
    let base = public_base(&state, &headers);
    let token = cast_token(&state.secret, id, Utc::now() + chrono::Duration::hours(4));
    let image_url = format!("{base}/api/assets/{id}/thumbnail?size=preview&castToken={token}");
    let (url, mime) = if asset.is_video() {
        (format!("{base}/api/assets/{id}/video?castToken={token}"), "video/mp4")
    } else {
        (image_url.clone(), "image/jpeg")
    };
    Ok(Json(json!({ "url": url, "mimeType": mime, "imageUrl": image_url })))
}
