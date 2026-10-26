//! DLNA/UPnP casting endpoints (smart TVs and media players on the LAN).

use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::access::require_read;
use crate::auth::{Auth, Principal, cast_token};
use crate::dlna::{Dlna, DlnaDevice};
use crate::error::{AppError, AppResult};
use crate::model::get_asset;
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/cast/devices", get(list).post(add))
        .route("/cast/devices/{id}/play", post(play))
        .route("/cast/devices/{id}/stop", post(stop))
        .route("/cast/devices/{id}/status", get(status))
}

#[derive(Deserialize)]
struct ListQuery {
    refresh: Option<bool>,
}

async fn list(State(state): State<SharedState>, _auth: Auth, Query(q): Query<ListQuery>) -> AppResult<Json<Vec<DlnaDevice>>> {
    if q.refresh.unwrap_or(false) || !state.dlna.scanned_recently() {
        state
            .dlna
            .discover(&state.http, Duration::from_secs(3))
            .await
            .map_err(|e| AppError::Unavailable(format!("device discovery failed: {e}")))?;
    }
    Ok(Json(state.dlna.devices()))
}

#[derive(Deserialize)]
struct AddDevice {
    location: String,
}

/// Adds a renderer by its UPnP description URL, e.g. http://192.168.1.20:9197/dmr.
async fn add(State(state): State<SharedState>, _auth: Auth, Json(body): Json<AddDevice>) -> AppResult<Json<DlnaDevice>> {
    let device = state.dlna.add(&state.http, body.location.trim()).await.map_err(|e| AppError::bad(e.to_string()))?;
    let mut stored: Vec<String> = sqlx::query_scalar("SELECT value FROM system_config WHERE key = 'dlna_devices'")
        .fetch_optional(&state.db)
        .await?
        .and_then(|v: Value| serde_json::from_value(v).ok())
        .unwrap_or_default();
    if !stored.contains(&device.location) {
        stored.push(device.location.clone());
        sqlx::query(
            "INSERT INTO system_config (key, value) VALUES ('dlna_devices', $1) ON CONFLICT (key) DO UPDATE SET value = $1",
        )
        .bind(json!(stored))
        .execute(&state.db)
        .await?;
    }
    Ok(Json(device))
}

/// Re-registers manually added renderers after a restart.
pub async fn restore_devices(state: &SharedState) {
    let stored: Vec<String> = sqlx::query_scalar("SELECT value FROM system_config WHERE key = 'dlna_devices'")
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|v: Value| serde_json::from_value(v).ok())
        .unwrap_or_default();
    for location in stored {
        if let Err(e) = state.dlna.add(&state.http, &location).await {
            tracing::info!("cast device {location} not reachable: {e:#}");
        }
    }
}

fn device(state: &SharedState, id: &str) -> AppResult<DlnaDevice> {
    state.dlna.get(id).ok_or_else(|| AppError::NotFound("cast device not found – refresh the device list".into()))
}

/// Base URL the renderer uses to fetch media: PH_PUBLIC_URL or this host's LAN address.
fn base_for(state: &SharedState, device: &DlnaDevice) -> AppResult<String> {
    if let Some(url) = &state.config.public_url {
        return Ok(url.clone());
    }
    let port = state.config.listen.rsplit(':').next().unwrap_or("2342");
    let ip = Dlna::local_ip_towards(device)
        .ok_or_else(|| AppError::bad("cannot determine a server address for this device – set PH_PUBLIC_URL"))?;
    Ok(format!("http://{ip}:{port}"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayBody {
    asset_id: Uuid,
}

async fn play(State(state): State<SharedState>, auth: Auth, Path(id): Path<String>, Json(body): Json<PlayBody>) -> AppResult<Json<Value>> {
    let device = device(&state, &id)?;
    let asset = get_asset(&state.db, body.asset_id).await?;
    let principal = Principal::User { user: auth.user.clone(), session_id: auth.session_id, unlocked: auth.unlocked };
    require_read(&state, &principal, &asset).await?;
    let base = base_for(&state, &device)?;
    let token = cast_token(&state.secret, asset.id, Utc::now() + chrono::Duration::hours(4));
    let (url, mime) = if asset.is_video() {
        (format!("{base}/api/assets/{}/video?castToken={token}", asset.id), "video/mp4")
    } else {
        (format!("{base}/api/assets/{}/thumbnail?size=preview&castToken={token}", asset.id), "image/jpeg")
    };
    Dlna::play(&state.http, &device, &url, mime, &asset.original_file_name)
        .await
        .map_err(|e| AppError::Unavailable(e.to_string()))?;
    metrics::counter!("pixelhort_cast_total", "kind" => "dlna").increment(1);
    Ok(Json(json!({ "device": device.name, "url": url, "mimeType": mime })))
}

async fn stop(State(state): State<SharedState>, _auth: Auth, Path(id): Path<String>) -> AppResult<Json<Value>> {
    let device = device(&state, &id)?;
    Dlna::stop(&state.http, &device).await.map_err(|e| AppError::Unavailable(e.to_string()))?;
    Ok(Json(json!({ "state": "STOPPED" })))
}

async fn status(State(state): State<SharedState>, _auth: Auth, Path(id): Path<String>) -> AppResult<Json<Value>> {
    let device = device(&state, &id)?;
    let s = Dlna::state(&state.http, &device).await.map_err(|e| AppError::Unavailable(e.to_string()))?;
    Ok(Json(json!({ "state": s })))
}
