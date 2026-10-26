//! Cron-style scheduler for periodic jobs (database backup, trash expiry,
//! library rescans) and metric gauges.

use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use cron::Schedule;
use serde_json::json;

use crate::state::SharedState;

fn next_after(expr: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
    Schedule::from_str(expr).ok()?.after(&after).next()
}

pub fn start(state: SharedState) {
    tokio::spawn(async move {
        let mut last = Utc::now();
        update_gauges(&state).await;
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let now = Utc::now();
            let sys = state.sys();
            let due = |expr: &str| next_after(expr, last).map(|t| t <= now).unwrap_or(false);
            if sys.backup.enabled && due(&sys.backup.cron) {
                if let Err(e) = state.jobs.enqueue("backup", None, json!({})).await {
                    tracing::error!("could not schedule backup: {e}");
                }
            }
            if due("0 30 0 * * *") {
                let _ = state.jobs.enqueue("cleanup", None, json!({ "kind": "trash" })).await;
            }
            if due("0 0 * * * *") {
                let _ = crate::jobs::start_queue(&state, "library", false).await;
            }
            last = now;
            update_gauges(&state).await;
        }
    });
}

async fn update_gauges(state: &SharedState) {
    let counts: Result<(i64, i64, i64, i64), _> = sqlx::query_as(
        "SELECT
            (SELECT COUNT(*) FROM assets WHERE asset_type = 'IMAGE' AND deleted_at IS NULL),
            (SELECT COUNT(*) FROM assets WHERE asset_type = 'VIDEO' AND deleted_at IS NULL AND visibility <> 'hidden'),
            (SELECT COUNT(*) FROM users),
            (SELECT COALESCE(SUM(file_size), 0)::BIGINT FROM assets WHERE library_id IS NULL)",
    )
    .fetch_one(&state.db)
    .await;
    if let Ok((images, videos, users, bytes)) = counts {
        metrics::gauge!("pixelhort_assets", "type" => "image").set(images as f64);
        metrics::gauge!("pixelhort_assets", "type" => "video").set(videos as f64);
        metrics::gauge!("pixelhort_users").set(users as f64);
        metrics::gauge!("pixelhort_storage_bytes").set(bytes as f64);
    }
    let jobs: Result<Vec<(String, String, i64)>, _> =
        sqlx::query_as("SELECT queue, status, COUNT(*) FROM jobs GROUP BY queue, status").fetch_all(&state.db).await;
    if let Ok(rows) = jobs {
        for (queue, _) in crate::jobs::QUEUES {
            for status in ["waiting", "active", "failed"] {
                let n = rows.iter().find(|(q, s, _)| q == queue && s == status).map(|r| r.2).unwrap_or(0);
                metrics::gauge!("pixelhort_jobs", "queue" => *queue, "status" => status).set(n as f64);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_next() {
        let base: DateTime<Utc> = "2024-01-01T01:00:00Z".parse().unwrap();
        let next = next_after("0 0 2 * * *", base).unwrap();
        assert_eq!(next.to_rfc3339(), "2024-01-01T02:00:00+00:00");
    }
}
