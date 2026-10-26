use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::geocode::Geocoder;
use crate::jobs::JobQueue;
use crate::sysconfig::SystemConfig;

pub type SharedState = Arc<AppState>;

pub struct AppState {
    pub db: PgPool,
    pub config: Config,
    pub sys: RwLock<SystemConfig>,
    pub secret: Vec<u8>,
    pub http: reqwest::Client,
    pub jobs: JobQueue,
    pub storage: Storage,
    pub geocoder: Geocoder,
    pub dlna: crate::dlna::Dlna,
    pub metrics: Option<PrometheusHandle>,
    /// Cache of CLIP text embeddings for recent search queries.
    pub clip_text_cache: Mutex<HashMap<String, Vec<f32>>>,
}

impl AppState {
    pub fn sys(&self) -> SystemConfig {
        self.sys.read().expect("config lock").clone()
    }

    pub fn ml_url(&self) -> Option<String> {
        let sys = self.sys();
        if !sys.ml.enabled {
            return None;
        }
        let url = if sys.ml.url.is_empty() { self.config.ml_url.clone()? } else { sys.ml.url };
        Some(url.trim_end_matches('/').to_string())
    }
}

/// Filesystem layout below `PH_DATA_DIR`.
#[derive(Debug, Clone)]
pub struct Storage {
    pub root: PathBuf,
}

fn shard(id: Uuid) -> (String, String) {
    let s = id.simple().to_string();
    (s[0..2].to_string(), s[2..4].to_string())
}

impl Storage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        for d in ["library", "upload/tmp", "thumbs", "encoded-video", "backups", "geodata"] {
            std::fs::create_dir_all(self.root.join(d))?;
        }
        Ok(())
    }

    pub fn tmp_file(&self) -> PathBuf {
        self.root.join("upload/tmp").join(Uuid::new_v4().to_string())
    }

    pub fn original_path(&self, user: Uuid, date: chrono::DateTime<chrono::Utc>, id: Uuid, ext: &str) -> PathBuf {
        self.root
            .join("library")
            .join(user.to_string())
            .join(date.format("%Y").to_string())
            .join(date.format("%m").to_string())
            .join(format!("{id}.{ext}"))
    }

    pub fn thumb_path(&self, user: Uuid, id: Uuid, kind: &str) -> PathBuf {
        let (a, b) = shard(id);
        self.root.join("thumbs").join(user.to_string()).join(a).join(b).join(format!("{id}-{kind}.jpg"))
    }

    pub fn encoded_video_path(&self, user: Uuid, id: Uuid) -> PathBuf {
        let (a, b) = shard(id);
        self.root.join("encoded-video").join(user.to_string()).join(a).join(b).join(format!("{id}.mp4"))
    }

    pub fn person_thumb_path(&self, user: Uuid, person: Uuid) -> PathBuf {
        self.root.join("thumbs").join(user.to_string()).join("people").join(format!("{person}.jpg"))
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn geodata_dir(&self) -> PathBuf {
        self.root.join("geodata")
    }

    /// True if `path` lives inside the managed upload library (and may be deleted).
    pub fn is_managed(&self, path: &Path) -> bool {
        path.starts_with(self.root.join("library"))
    }
}
