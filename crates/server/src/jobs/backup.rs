//! Database backups (pg_dump, gzip, retention), trash expiry and file cleanup.

use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;
use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::Value;
use tokio::io::AsyncReadExt;

use crate::state::SharedState;

pub async fn run(state: &SharedState) -> anyhow::Result<()> {
    let dir = state.storage.backups_dir();
    tokio::fs::create_dir_all(&dir).await?;
    let name = format!("pixelhort-db-{}.sql.gz", Utc::now().format("%Y%m%dT%H%M%S"));
    let path = dir.join(&name);
    let tmp = dir.join(format!("{name}.part"));

    let mut child = tokio::process::Command::new("pg_dump")
        .arg("--dbname")
        .arg(&state.config.database_url)
        .args(["--clean", "--if-exists", "--no-owner", "--no-privileges"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().expect("stdout");
    let file = std::fs::File::create(&tmp)?;
    let mut gz = GzEncoder::new(file, Compression::default());
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = stdout.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        gz.write_all(&buf[..n])?;
    }
    gz.finish()?;
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        let _ = tokio::fs::remove_file(&tmp).await;
        anyhow::bail!("pg_dump failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    tokio::fs::rename(&tmp, &path).await?;
    tracing::info!("database backup written: {name}");
    metrics::counter!("pixelhort_db_backups_total").increment(1);
    prune(state).await
}

pub async fn list(state: &SharedState) -> anyhow::Result<Vec<(String, u64, chrono::DateTime<Utc>)>> {
    let mut out = Vec::new();
    let mut rd = match tokio::fs::read_dir(state.storage.backups_dir()).await {
        Ok(rd) => rd,
        Err(_) => return Ok(out),
    };
    while let Some(e) = rd.next_entry().await? {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".sql.gz") {
            continue;
        }
        let meta = e.metadata().await?;
        let created = meta.modified().map(chrono::DateTime::<Utc>::from).unwrap_or_else(|_| Utc::now());
        out.push((name, meta.len(), created));
    }
    out.sort_by(|a, b| b.2.cmp(&a.2));
    Ok(out)
}

async fn prune(state: &SharedState) -> anyhow::Result<()> {
    let keep = state.sys().backup.keep.max(1);
    for (name, _, _) in list(state).await?.into_iter().skip(keep) {
        let _ = tokio::fs::remove_file(state.storage.backups_dir().join(name)).await;
    }
    Ok(())
}

/// Permanently deletes assets that stayed in the trash longer than configured.
pub async fn empty_expired_trash(state: &SharedState) -> anyhow::Result<()> {
    let days = state.sys().trash.days.max(0);
    let ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM assets WHERE deleted_at IS NOT NULL AND deleted_at < now() - make_interval(days => $1)",
    )
    .bind(days as i32)
    .fetch_all(&state.db)
    .await?;
    if !ids.is_empty() {
        tracing::info!("removing {} expired trashed assets", ids.len());
        crate::routes::assets::delete_permanently(state, &ids).await?;
    }
    sqlx::query("DELETE FROM oauth_states WHERE created_at < now() - interval '1 hour'").execute(&state.db).await?;
    Ok(())
}

/// Removes files listed in a cleanup job payload (`{"files": [...]}`).
pub async fn delete_files(payload: &Value) -> anyhow::Result<()> {
    for f in payload["files"].as_array().into_iter().flatten().filter_map(|v| v.as_str()) {
        let p = PathBuf::from(f);
        if let Err(e) = tokio::fs::remove_file(&p).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("could not delete {f}: {e}");
            }
        }
    }
    Ok(())
}
