//! Postgres-backed background job queue (`SELECT ... FOR UPDATE SKIP LOCKED`).

pub mod backup;
pub mod library;
pub mod metadata;
pub mod ml;
pub mod scheduler;
pub mod thumbnails;
pub mod video;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde_json::{Value, json};
use sqlx::{FromRow, PgPool};
use tokio::sync::Notify;
use uuid::Uuid;

use crate::state::SharedState;

/// Queue name and number of concurrent workers.
pub const QUEUES: &[(&str, usize)] = &[
    ("metadata", 4),
    ("thumbnails", 4),
    ("video", 1),
    ("smart-search", 2),
    ("faces", 2),
    ("facial-recognition", 1),
    ("ocr", 1),
    ("objects", 2),
    ("duplicates", 1),
    ("library", 1),
    ("backup", 1),
    ("cleanup", 1),
];

pub const MAX_ATTEMPTS: i32 = 3;

/// Upper limit for the worker counts set in the admin settings.
pub const MAX_WORKERS: usize = 8;

/// Worker count from the admin settings, for the queues that have one.
fn configured_workers(state: &SharedState, queue: &str) -> Option<usize> {
    let j = state.sys().jobs;
    match queue {
        "metadata" => Some(j.metadata),
        "thumbnails" => Some(j.thumbnails),
        "video" => Some(j.video),
        _ => None,
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct Job {
    pub id: i64,
    pub queue: String,
    pub asset_id: Option<Uuid>,
    pub payload: Value,
    pub attempts: i32,
}

pub struct JobQueue {
    db: PgPool,
    notify: HashMap<&'static str, Arc<Notify>>,
    paused: RwLock<HashSet<String>>,
    completed: HashMap<&'static str, AtomicU64>,
}

impl JobQueue {
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            notify: QUEUES.iter().map(|(q, _)| (*q, Arc::new(Notify::new()))).collect(),
            paused: RwLock::new(HashSet::new()),
            completed: QUEUES.iter().map(|(q, _)| (*q, AtomicU64::new(0))).collect(),
        }
    }

    pub fn is_queue(name: &str) -> bool {
        QUEUES.iter().any(|(q, _)| *q == name)
    }

    pub async fn load_state(&self) -> anyhow::Result<()> {
        let rows: Vec<(String, bool, i64)> = sqlx::query_as("SELECT queue, paused, completed FROM queue_state")
            .fetch_all(&self.db)
            .await?;
        let mut paused = self.paused.write().expect("lock");
        for (q, p, c) in rows {
            if p {
                paused.insert(q.clone());
            }
            if let Some(counter) = self.completed.get(q.as_str()) {
                counter.store(c as u64, Ordering::Relaxed);
            }
        }
        Ok(())
    }

    pub fn wake(&self, queue: &str) {
        if let Some(n) = self.notify.get(queue) {
            n.notify_waiters();
            n.notify_one();
        }
    }

    pub async fn enqueue(&self, queue: &str, asset_id: Option<Uuid>, payload: Value) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO jobs (queue, asset_id, payload) VALUES ($1, $2, $3)
             ON CONFLICT (queue, asset_id) WHERE status = 'waiting' AND asset_id IS NOT NULL DO NOTHING",
        )
        .bind(queue)
        .bind(asset_id)
        .bind(payload)
        .execute(&self.db)
        .await?;
        self.wake(queue);
        Ok(())
    }

    pub async fn enqueue_asset(&self, queue: &str, asset_id: Uuid) -> anyhow::Result<()> {
        self.enqueue(queue, Some(asset_id), json!({})).await
    }

    /// Enqueues jobs for every asset matching `where_sql` (a condition on `a`).
    pub async fn enqueue_where(&self, queue: &str, where_sql: &str) -> anyhow::Result<u64> {
        let sql = format!(
            "INSERT INTO jobs (queue, asset_id) SELECT $1, a.id FROM assets a WHERE {where_sql}
             ON CONFLICT (queue, asset_id) WHERE status = 'waiting' AND asset_id IS NOT NULL DO NOTHING"
        );
        let n = sqlx::query(&sql).bind(queue).execute(&self.db).await?.rows_affected();
        self.wake(queue);
        Ok(n)
    }

    pub fn is_paused(&self, queue: &str) -> bool {
        self.paused.read().expect("lock").contains(queue)
    }

    pub async fn set_paused(&self, queue: &str, paused: bool) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO queue_state (queue, paused) VALUES ($1, $2) ON CONFLICT (queue) DO UPDATE SET paused = $2",
        )
        .bind(queue)
        .bind(paused)
        .execute(&self.db)
        .await?;
        {
            let mut set = self.paused.write().expect("lock");
            if paused {
                set.insert(queue.to_string());
            } else {
                set.remove(queue);
            }
        }
        self.wake(queue);
        Ok(())
    }

    pub fn completed(&self, queue: &str) -> u64 {
        self.completed.get(queue).map(|c| c.load(Ordering::Relaxed)).unwrap_or(0)
    }

    async fn dequeue(&self, queue: &str) -> sqlx::Result<Option<Job>> {
        sqlx::query_as(
            "UPDATE jobs SET status = 'active', attempts = attempts + 1
             WHERE id = (SELECT id FROM jobs WHERE queue = $1 AND status = 'waiting' AND run_at <= now()
                         ORDER BY run_at, id LIMIT 1 FOR UPDATE SKIP LOCKED)
             RETURNING id, queue, asset_id, payload, attempts",
        )
        .bind(queue)
        .fetch_optional(&self.db)
        .await
    }

    async fn finish(&self, job: &Job, result: &anyhow::Result<()>) -> sqlx::Result<()> {
        match result {
            Ok(()) => {
                sqlx::query("DELETE FROM jobs WHERE id = $1").bind(job.id).execute(&self.db).await?;
                if let Some(c) = self.completed.get(job.queue.as_str()) {
                    c.fetch_add(1, Ordering::Relaxed);
                }
                sqlx::query(
                    "INSERT INTO queue_state (queue, completed) VALUES ($1, 1)
                     ON CONFLICT (queue) DO UPDATE SET completed = queue_state.completed + 1",
                )
                .bind(&job.queue)
                .execute(&self.db)
                .await?;
                metrics::counter!("pixelhort_jobs_completed_total", "queue" => job.queue.clone()).increment(1);
            }
            Err(e) => {
                let msg = format!("{e:#}");
                tracing::warn!(queue = %job.queue, asset = ?job.asset_id, attempt = job.attempts, "job failed: {msg}");
                metrics::counter!("pixelhort_jobs_failed_total", "queue" => job.queue.clone()).increment(1);
                if job.attempts < MAX_ATTEMPTS {
                    let retry = sqlx::query(
                        "UPDATE jobs SET status = 'waiting', error = $2,
                         run_at = now() + make_interval(secs => $3) WHERE id = $1",
                    )
                    .bind(job.id)
                    .bind(&msg)
                    .bind(30.0 * job.attempts as f64)
                    .execute(&self.db)
                    .await;
                    if retry.is_err() {
                        // another waiting job for the same asset exists already
                        sqlx::query("DELETE FROM jobs WHERE id = $1").bind(job.id).execute(&self.db).await?;
                    }
                } else {
                    sqlx::query("UPDATE jobs SET status = 'failed', error = $2 WHERE id = $1")
                        .bind(job.id)
                        .bind(&msg)
                        .execute(&self.db)
                        .await?;
                }
            }
        }
        Ok(())
    }

    /// Re-queues jobs that were active when the server stopped.
    pub async fn recover(&self) -> sqlx::Result<()> {
        sqlx::query(
            "DELETE FROM jobs j WHERE status = 'active' AND EXISTS (
                SELECT 1 FROM jobs w WHERE w.queue = j.queue AND w.asset_id = j.asset_id AND w.status = 'waiting')",
        )
        .execute(&self.db)
        .await?;
        sqlx::query("UPDATE jobs SET status = 'waiting' WHERE status = 'active'").execute(&self.db).await?;
        Ok(())
    }
}

/// Executes one job.
async fn handle(state: &SharedState, job: &Job) -> anyhow::Result<()> {
    let asset = || job.asset_id.ok_or_else(|| anyhow::anyhow!("job without asset"));
    match job.queue.as_str() {
        "metadata" => metadata::run(state, asset()?).await,
        "thumbnails" => thumbnails::run(state, asset()?).await,
        "video" => video::run(state, asset()?).await,
        "smart-search" => ml::smart_search(state, asset()?).await,
        "faces" => ml::detect_faces(state, asset()?).await,
        "facial-recognition" => ml::recognize_faces(state, asset()?).await,
        "ocr" => ml::ocr(state, asset()?).await,
        "objects" => ml::objects(state, asset()?).await,
        "duplicates" => ml::duplicates(state, asset()?).await,
        "library" => {
            let id = job.payload["libraryId"].as_str().and_then(|s| Uuid::parse_str(s).ok());
            library::scan(state, id.ok_or_else(|| anyhow::anyhow!("libraryId missing"))?).await
        }
        "backup" => backup::run(state).await,
        "cleanup" => match job.payload["kind"].as_str() {
            Some("trash") => backup::empty_expired_trash(state).await,
            Some("files") => backup::delete_files(&job.payload).await,
            _ => Ok(()),
        },
        other => anyhow::bail!("unknown queue {other}"),
    }
}

pub fn start_workers(state: SharedState) {
    for (queue, concurrency) in QUEUES {
        // Configurable queues get the maximum; workers above the setting stay idle.
        let spawn = if configured_workers(&state, queue).is_some() { MAX_WORKERS } else { *concurrency };
        for worker in 0..spawn {
            let state = state.clone();
            let queue = *queue;
            tokio::spawn(async move {
                let notify = state.jobs.notify[queue].clone();
                loop {
                    let enabled = configured_workers(&state, queue).is_none_or(|n| worker < n);
                    if state.jobs.is_paused(queue) || !enabled {
                        tokio::select! {
                            _ = notify.notified() => {}
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                        }
                        continue;
                    }
                    match state.jobs.dequeue(queue).await {
                        Ok(Some(job)) => {
                            let started = std::time::Instant::now();
                            let result = handle(&state, &job).await;
                            metrics::histogram!("pixelhort_job_duration_seconds", "queue" => queue)
                                .record(started.elapsed().as_secs_f64());
                            if let Err(e) = state.jobs.finish(&job, &result).await {
                                tracing::error!("failed to finish job: {e}");
                            }
                        }
                        Ok(None) => {
                            tokio::select! {
                                _ = notify.notified() => {}
                                _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                            }
                        }
                        Err(e) => {
                            tracing::error!("dequeue failed: {e}");
                            tokio::time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            });
        }
    }
}

/// Handles the admin "start" command: queue every asset missing that step.
pub async fn start_queue(state: &SharedState, queue: &str, force: bool) -> anyhow::Result<u64> {
    let live = "a.deleted_at IS NULL AND NOT a.is_offline";
    let ml = "a.deleted_at IS NULL AND a.visibility <> 'hidden' AND a.thumbnails_at IS NOT NULL";
    let missing = |col: &str| if force { String::new() } else { format!(" AND a.{col} IS NULL") };
    let q = &state.jobs;
    let n = match queue {
        "metadata" => q.enqueue_where(queue, &format!("{live}{}", missing("metadata_at"))).await?,
        "thumbnails" => q.enqueue_where(queue, &format!("{live}{}", missing("thumbnails_at"))).await?,
        "video" => q.enqueue_where(queue, &format!("{live} AND a.asset_type = 'VIDEO'{}", missing("video_at"))).await?,
        "smart-search" => q.enqueue_where(queue, &format!("{ml}{}", missing("smart_search_at"))).await?,
        "faces" => q.enqueue_where(queue, &format!("{ml}{}", missing("faces_at"))).await?,
        "ocr" => q.enqueue_where(queue, &format!("{ml} AND a.asset_type = 'IMAGE'{}", missing("ocr_at"))).await?,
        "objects" => q.enqueue_where(queue, &format!("{ml}{}", missing("objects_at"))).await?,
        "duplicates" => {
            q.enqueue_where(queue, &format!("{ml} AND a.smart_search_at IS NOT NULL{}", missing("duplicates_at"))).await?
        }
        "facial-recognition" => {
            let cond = if force {
                "EXISTS (SELECT 1 FROM faces f WHERE f.asset_id = a.id)"
            } else {
                "EXISTS (SELECT 1 FROM faces f WHERE f.asset_id = a.id AND f.person_id IS NULL)"
            };
            q.enqueue_where(queue, &format!("a.deleted_at IS NULL AND {cond}")).await?
        }
        "library" => {
            let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM libraries").fetch_all(&state.db).await?;
            for id in &ids {
                q.enqueue("library", None, json!({ "libraryId": id })).await?;
            }
            ids.len() as u64
        }
        "backup" => {
            q.enqueue("backup", None, json!({})).await?;
            1
        }
        "cleanup" => {
            q.enqueue("cleanup", None, json!({ "kind": "trash" })).await?;
            1
        }
        _ => anyhow::bail!("unknown queue"),
    };
    Ok(n)
}

/// Enqueues the follow-up steps after thumbnails exist.
pub async fn enqueue_ml(state: &SharedState, asset_id: Uuid, asset_type: &str) -> anyhow::Result<()> {
    if state.ml_url().is_none() {
        return Ok(());
    }
    let sys = state.sys();
    state.jobs.enqueue_asset("smart-search", asset_id).await?;
    state.jobs.enqueue_asset("faces", asset_id).await?;
    if sys.ml.ocr_enabled && asset_type == "IMAGE" {
        state.jobs.enqueue_asset("ocr", asset_id).await?;
    }
    if sys.ml.objects_enabled {
        state.jobs.enqueue_asset("objects", asset_id).await?;
    }
    Ok(())
}
