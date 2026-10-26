use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use pixelhort_common::{ServerFeatures, ServerInfo};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::{AdminAuth, Auth};
use crate::error::{AppError, AppResult};
use crate::jobs::{self, JobQueue, QUEUES};
use crate::state::SharedState;
use crate::sysconfig::{self, SystemConfig, merge_json};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/server/ping", get(|| async { Json(json!({"res": "pong"})) }))
        .route("/server/info", get(info))
        .route("/server/statistics", get(statistics))
        .route("/admin/config", get(get_config).put(put_config))
        .route("/admin/backups", get(list_backups).post(create_backup))
        .route("/admin/backups/{name}", get(download_backup).delete(delete_backup))
        .route("/jobs", get(job_status))
        .route("/jobs/{queue}", post(job_command))
}

async fn info(State(state): State<SharedState>) -> Json<ServerInfo> {
    let sys = state.sys();
    let ml = state.ml_url().is_some();
    Json(ServerInfo {
        version: pixelhort_common::API_VERSION.to_string(),
        features: ServerFeatures {
            smart_search: ml,
            faces: ml,
            ocr: ml && sys.ml.ocr_enabled,
            objects: ml && sys.ml.objects_enabled,
            oauth: sys.oauth.enabled,
            reverse_geocoding: sys.reverse_geocoding.enabled && state.geocoder.is_loaded(),
            cast: true,
        },
        public_url: state.config.public_url.clone(),
    })
}

async fn statistics(State(state): State<SharedState>, _a: AdminAuth) -> AppResult<Json<Value>> {
    let rows: Vec<(uuid::Uuid, String, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.id, u.name,
            COUNT(a.id) FILTER (WHERE a.asset_type = 'IMAGE'),
            COUNT(a.id) FILTER (WHERE a.asset_type = 'VIDEO' AND a.visibility <> 'hidden'),
            COALESCE(SUM(a.file_size), 0)::BIGINT
         FROM users u LEFT JOIN assets a ON a.owner_id = u.id AND a.deleted_at IS NULL
         GROUP BY u.id, u.name ORDER BY u.name",
    )
    .fetch_all(&state.db)
    .await?;
    let photos: i64 = rows.iter().map(|r| r.2).sum();
    let videos: i64 = rows.iter().map(|r| r.3).sum();
    let usage: i64 = rows.iter().map(|r| r.4).sum();
    Ok(Json(json!({
        "photos": photos, "videos": videos, "usageBytes": usage,
        "users": rows.iter().map(|(id, name, p, v, u)| json!({
            "userId": id, "name": name, "photos": p, "videos": v, "usageBytes": u
        })).collect::<Vec<_>>(),
    })))
}

async fn get_config(State(state): State<SharedState>, _a: AdminAuth) -> Json<SystemConfig> {
    Json(state.sys())
}

async fn put_config(State(state): State<SharedState>, _a: AdminAuth, Json(patch): Json<Value>) -> AppResult<Json<SystemConfig>> {
    let mut value = serde_json::to_value(state.sys()).map_err(|e| AppError::Internal(e.into()))?;
    merge_json(&mut value, patch);
    let cfg: SystemConfig = serde_json::from_value(value).map_err(|e| AppError::bad(format!("invalid config: {e}")))?;
    if cfg.backup.enabled {
        cfg.backup.cron.parse::<cron::Schedule>().map_err(|e| AppError::bad(format!("invalid cron expression: {e}")))?;
    }
    sysconfig::save(&state.db, &cfg).await?;
    *state.sys.write().expect("config lock") = cfg.clone();
    Ok(Json(cfg))
}

async fn list_backups(State(state): State<SharedState>, _a: AdminAuth) -> AppResult<Json<Value>> {
    let list = jobs::backup::list(&state).await?;
    Ok(Json(json!(list
        .into_iter()
        .map(|(name, size, created)| json!({"name": name, "size": size, "createdAt": created}))
        .collect::<Vec<_>>())))
}

async fn create_backup(State(state): State<SharedState>, _a: AdminAuth) -> AppResult<StatusCode> {
    state.jobs.enqueue("backup", None, json!({})).await?;
    Ok(StatusCode::ACCEPTED)
}

fn backup_path(state: &SharedState, name: &str) -> AppResult<std::path::PathBuf> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || !name.ends_with(".sql.gz") {
        return Err(AppError::bad("invalid backup name"));
    }
    let p = state.storage.backups_dir().join(name);
    if !p.exists() {
        return Err(AppError::not_found());
    }
    Ok(p)
}

async fn download_backup(State(state): State<SharedState>, _a: AdminAuth, Path(name): Path<String>) -> AppResult<Response> {
    let p = backup_path(&state, &name)?;
    let file = tokio::fs::File::open(&p).await?;
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(file));
    Ok((
        [
            (header::CONTENT_TYPE, "application/gzip".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
        ],
        body,
    )
        .into_response())
}

async fn delete_backup(State(state): State<SharedState>, _a: AdminAuth, Path(name): Path<String>) -> AppResult<StatusCode> {
    let p = backup_path(&state, &name)?;
    tokio::fs::remove_file(p).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn job_status(State(state): State<SharedState>, _a: Auth) -> AppResult<Json<Value>> {
    let rows: Vec<(String, String, i64)> =
        sqlx::query_as("SELECT queue, status, COUNT(*) FROM jobs GROUP BY queue, status").fetch_all(&state.db).await?;
    let mut out = serde_json::Map::new();
    for (queue, _) in QUEUES {
        let count = |s: &str| rows.iter().find(|(q, st, _)| q == queue && st == s).map(|r| r.2).unwrap_or(0);
        out.insert(
            queue.to_string(),
            json!({
                "active": count("active"),
                "waiting": count("waiting"),
                "failed": count("failed"),
                "completed": state.jobs.completed(queue),
                "paused": state.jobs.is_paused(queue),
            }),
        );
    }
    Ok(Json(Value::Object(out)))
}

#[derive(Deserialize)]
struct JobCommand {
    command: String,
    #[serde(default)]
    force: bool,
}

async fn job_command(
    State(state): State<SharedState>,
    _a: AdminAuth,
    Path(queue): Path<String>,
    Json(cmd): Json<JobCommand>,
) -> AppResult<StatusCode> {
    if !JobQueue::is_queue(&queue) {
        return Err(AppError::not_found());
    }
    match cmd.command.as_str() {
        "start" => {
            jobs::start_queue(&state, &queue, cmd.force).await?;
        }
        "pause" => state.jobs.set_paused(&queue, true).await?,
        "resume" => state.jobs.set_paused(&queue, false).await?,
        "clear-failed" => {
            sqlx::query("DELETE FROM jobs WHERE queue = $1 AND status = 'failed'").bind(&queue).execute(&state.db).await?;
        }
        "retry-failed" => {
            sqlx::query(
                "DELETE FROM jobs f WHERE f.queue = $1 AND f.status = 'failed' AND EXISTS (
                    SELECT 1 FROM jobs w WHERE w.queue = f.queue AND w.asset_id = f.asset_id AND w.status = 'waiting')",
            )
            .bind(&queue)
            .execute(&state.db)
            .await?;
            sqlx::query("UPDATE jobs SET status = 'waiting', attempts = 0, run_at = now() WHERE queue = $1 AND status = 'failed'")
                .bind(&queue)
                .execute(&state.db)
                .await?;
        }
        _ => return Err(AppError::bad("unknown command")),
    }
    Ok(StatusCode::NO_CONTENT)
}
