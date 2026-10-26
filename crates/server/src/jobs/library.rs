//! External libraries: index existing folders (read-only) and watch them for changes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use globset::{Glob, GlobSet, GlobSetBuilder};
use notify::{RecursiveMode, Watcher};
use serde_json::json;
use sha1::{Digest, Sha1};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::media;
use crate::state::SharedState;

#[derive(Debug, sqlx::FromRow)]
pub struct LibraryRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub import_paths: Vec<String>,
    pub exclusion_patterns: Vec<String>,
    pub watch: bool,
    pub refreshed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub fn build_globs(patterns: &[String]) -> anyhow::Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for p in patterns.iter().filter(|p| !p.trim().is_empty()) {
        b.add(Glob::new(p.trim())?);
    }
    Ok(b.build()?)
}

pub async fn sha1_file(path: &Path) -> std::io::Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_files(roots: Vec<String>, exclude: GlobSet) -> Vec<(PathBuf, u64, DateTime<Utc>)> {
    let mut out = Vec::new();
    for root in roots {
        for entry in walkdir::WalkDir::new(&root).follow_links(true).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if exclude.is_match(path) {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || pixelhort_common::media_type_for(name).is_none() {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let mtime = meta.modified().map(DateTime::<Utc>::from).unwrap_or_else(|_| Utc::now());
            out.push((path.to_path_buf(), meta.len(), mtime));
        }
    }
    out
}

pub async fn scan(state: &SharedState, library_id: Uuid) -> anyhow::Result<()> {
    let lib: LibraryRow = sqlx::query_as("SELECT * FROM libraries WHERE id = $1")
        .bind(library_id)
        .fetch_one(&state.db)
        .await?;
    let exclude = build_globs(&lib.exclusion_patterns)?;
    let roots = lib.import_paths.clone();
    let files = tokio::task::spawn_blocking(move || collect_files(roots, exclude)).await?;
    tracing::info!("library {} scan: {} files", lib.name, files.len());

    let existing: Vec<(Uuid, String, DateTime<Utc>, bool)> = sqlx::query_as(
        "SELECT id, original_path, file_modified_at, is_offline FROM assets WHERE library_id = $1",
    )
    .bind(library_id)
    .fetch_all(&state.db)
    .await?;
    let known: HashMap<String, (Uuid, DateTime<Utc>, bool)> =
        existing.into_iter().map(|(id, p, m, off)| (p, (id, m, off))).collect();
    let mut seen = HashSet::new();

    for (path, size, mtime) in files {
        let path_str = path.to_string_lossy().to_string();
        seen.insert(path_str.clone());
        match known.get(&path_str) {
            Some((id, old_mtime, offline)) => {
                if (mtime - *old_mtime).num_seconds().abs() > 1 || *offline {
                    let checksum = sha1_file(&path).await?;
                    sqlx::query(
                        "UPDATE assets SET checksum = $2, file_size = $3, file_modified_at = $4, is_offline = FALSE,
                            updated_at = now() WHERE id = $1",
                    )
                    .bind(id)
                    .bind(checksum)
                    .bind(size as i64)
                    .bind(mtime)
                    .execute(&state.db)
                    .await?;
                    state.jobs.enqueue_asset("metadata", *id).await?;
                }
            }
            None => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
                let Some(kind) = pixelhort_common::media_type_for(&name) else { continue };
                let checksum = match sha1_file(&path).await {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("cannot read {path_str}: {e}");
                        continue;
                    }
                };
                let id = Uuid::new_v4();
                let sidecar = [PathBuf::from(format!("{path_str}.xmp")), path.with_extension("xmp")]
                    .into_iter()
                    .find(|p| p.exists())
                    .map(|p| p.to_string_lossy().to_string());
                sqlx::query(
                    "INSERT INTO assets (id, owner_id, library_id, asset_type, original_path, original_file_name,
                        original_mime_type, sidecar_path, checksum, file_size, file_created_at, file_modified_at,
                        local_date_time, is_raw)
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11,$11,$12) ON CONFLICT DO NOTHING",
                )
                .bind(id)
                .bind(lib.owner_id)
                .bind(library_id)
                .bind(kind)
                .bind(&path_str)
                .bind(&name)
                .bind(media::mime_for(&path))
                .bind(sidecar)
                .bind(checksum)
                .bind(size as i64)
                .bind(mtime)
                .bind(pixelhort_common::is_raw(&name))
                .execute(&state.db)
                .await?;
                state.jobs.enqueue_asset("metadata", id).await?;
            }
        }
    }
    let missing: Vec<Uuid> = known
        .iter()
        .filter(|(p, (_, _, offline))| !offline && !seen.contains(*p))
        .map(|(_, (id, _, _))| *id)
        .collect();
    if !missing.is_empty() {
        tracing::info!("library {}: {} files went offline", lib.name, missing.len());
        sqlx::query("UPDATE assets SET is_offline = TRUE, updated_at = now() WHERE id = ANY($1)")
            .bind(&missing)
            .execute(&state.db)
            .await?;
    }
    sqlx::query("UPDATE libraries SET refreshed_at = now() WHERE id = $1").bind(library_id).execute(&state.db).await?;
    Ok(())
}

/// Watches all libraries with `watch = true` and queues a rescan on changes (debounced).
pub fn start_watcher(state: SharedState) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = watch_round(&state).await {
                tracing::warn!("library watcher: {e:#}");
            }
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

async fn watch_round(state: &SharedState) -> anyhow::Result<()> {
    let libs: Vec<LibraryRow> = sqlx::query_as("SELECT * FROM libraries WHERE watch").fetch_all(&state.db).await?;
    if libs.is_empty() {
        return Ok(());
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            // Reading files (e.g. our own scan) emits access events; only react to real changes.
            if !matches!(ev.kind, notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)) {
                return;
            }
            if matches!(ev.kind, notify::EventKind::Modify(notify::event::ModifyKind::Metadata(_))) {
                return;
            }
            for p in ev.paths {
                let _ = tx.send(p);
            }
        }
    })?;
    let mut roots: Vec<(PathBuf, Uuid)> = Vec::new();
    for lib in &libs {
        for p in &lib.import_paths {
            if watcher.watch(Path::new(p), RecursiveMode::Recursive).is_ok() {
                roots.push((PathBuf::from(p), lib.id));
            }
        }
    }
    let config_version = libs.iter().map(|l| l.id.to_string() + &l.import_paths.join(",")).collect::<String>();
    let mut pending: HashSet<Uuid> = HashSet::new();
    loop {
        tokio::select! {
            Some(p) = rx.recv() => {
                for (root, id) in &roots {
                    if p.starts_with(root) {
                        pending.insert(*id);
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                for id in pending.drain() {
                    state.jobs.enqueue("library", None, json!({ "libraryId": id })).await?;
                }
                // Reload when libraries changed.
                let libs: Vec<LibraryRow> = sqlx::query_as("SELECT * FROM libraries WHERE watch").fetch_all(&state.db).await?;
                let now = libs.iter().map(|l| l.id.to_string() + &l.import_paths.join(",")).collect::<String>();
                if now != config_version {
                    return Ok(());
                }
            }
        }
    }
}
