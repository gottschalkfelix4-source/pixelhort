//! Runtime-editable system configuration, stored as JSON in `system_config`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SystemConfig {
    pub ml: MlConfig,
    pub backup: BackupConfig,
    pub video: VideoConfig,
    pub trash: TrashConfig,
    pub reverse_geocoding: Toggle,
    pub password_login: Toggle,
    pub oauth: OAuthConfig,
    pub server: ServerConfig,
    pub user: UserDefaults,
    pub library: LibraryConfig,
    pub image: ImageConfig,
    pub map: MapConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfig {
    /// External URL for share links and casting; overrides `PH_PUBLIC_URL` when set.
    pub external_domain: String,
    /// Shown on the login page.
    pub login_page_message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserDefaults {
    /// Quota for new users (admin-created without an explicit quota, OAuth auto-registration).
    pub default_quota_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LibraryConfig {
    pub scan_enabled: bool,
    pub scan_cron: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageConfig {
    pub preview_size: u32,
    pub preview_quality: u8,
    pub thumbnail_size: u32,
    pub thumbnail_quality: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MapConfig {
    pub enabled: bool,
    pub tile_url: String,
    pub attribution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MlConfig {
    pub enabled: bool,
    pub url: String,
    pub clip_model: String,
    pub face_min_score: f32,
    pub face_max_distance: f32,
    pub face_min_faces: i64,
    pub duplicate_max_distance: f32,
    pub ocr_enabled: bool,
    pub objects_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackupConfig {
    pub enabled: bool,
    pub cron: String,
    pub keep: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoConfig {
    pub target_resolution: u32,
    pub crf: u32,
    pub preset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrashConfig {
    pub days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Toggle {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OAuthConfig {
    pub enabled: bool,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scope: String,
    pub button_text: String,
    pub auto_register: bool,
    pub mobile_redirect_uri: String,
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

impl Default for MlConfig {
    fn default() -> Self {
        let url = env("PH_ML_URL").unwrap_or_default();
        Self {
            enabled: !url.is_empty(),
            url,
            clip_model: "ViT-B-32__openai".into(),
            face_min_score: 0.7,
            face_max_distance: 0.5,
            face_min_faces: 3,
            duplicate_max_distance: 0.01,
            ocr_enabled: true,
            objects_enabled: true,
        }
    }
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cron: env("PH_BACKUP_CRON").unwrap_or_else(|| "0 0 2 * * *".into()),
            keep: env("PH_BACKUP_KEEP").and_then(|v| v.parse().ok()).unwrap_or(14),
        }
    }
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self { target_resolution: 720, crf: 23, preset: "veryfast".into() }
    }
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self { scan_enabled: true, scan_cron: "0 0 * * * *".into() }
    }
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self { preview_size: 1440, preview_quality: 82, thumbnail_size: 250, thumbnail_quality: 78 }
    }
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tile_url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".into(),
            attribution: "© <a href=\"https://www.openstreetmap.org/copyright\" target=\"_blank\">OpenStreetMap</a>".into(),
        }
    }
}

impl Default for TrashConfig {
    fn default() -> Self {
        Self { days: 30 }
    }
}

impl Default for Toggle {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for OAuthConfig {
    fn default() -> Self {
        let issuer = env("PH_OAUTH_ISSUER_URL").unwrap_or_default();
        Self {
            enabled: !issuer.is_empty(),
            issuer_url: issuer,
            client_id: env("PH_OAUTH_CLIENT_ID").unwrap_or_default(),
            client_secret: env("PH_OAUTH_CLIENT_SECRET").unwrap_or_default(),
            scope: env("PH_OAUTH_SCOPE").unwrap_or_else(|| "openid email profile".into()),
            button_text: env("PH_OAUTH_BUTTON_TEXT").unwrap_or_else(|| "Login with SSO".into()),
            auto_register: env("PH_OAUTH_AUTO_REGISTER").map(|v| v != "false").unwrap_or(true),
            mobile_redirect_uri: env("PH_OAUTH_MOBILE_REDIRECT_URI")
                .unwrap_or_else(|| "app.pixelhort:/oauth-callback".into()),
        }
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            ml: MlConfig::default(),
            backup: BackupConfig::default(),
            video: VideoConfig::default(),
            trash: TrashConfig::default(),
            reverse_geocoding: Toggle {
                enabled: env("PH_REVERSE_GEOCODING").map(|v| v != "false").unwrap_or(true),
            },
            password_login: Toggle::default(),
            oauth: OAuthConfig::default(),
            server: ServerConfig::default(),
            user: UserDefaults::default(),
            library: LibraryConfig::default(),
            image: ImageConfig::default(),
            map: MapConfig::default(),
        }
    }
}

/// Recursively merges `patch` into `base` (objects merge, everything else replaces).
pub fn merge_json(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                merge_json(b.entry(k).or_insert(Value::Null), v);
            }
        }
        (b, p) => *b = p,
    }
}

pub async fn load(db: &PgPool) -> anyhow::Result<SystemConfig> {
    let stored: Option<Value> =
        sqlx::query_scalar("SELECT value FROM system_config WHERE key = 'config'")
            .fetch_optional(db)
            .await?;
    let mut value = serde_json::to_value(SystemConfig::default())?;
    if let Some(stored) = stored {
        merge_json(&mut value, stored);
    }
    Ok(serde_json::from_value(value)?)
}

pub async fn save(db: &PgPool, config: &SystemConfig) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO system_config (key, value) VALUES ('config', $1)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(serde_json::to_value(config)?)
    .execute(db)
    .await?;
    Ok(())
}

/// Returns a random secret used for signing cast/share tokens, created on first start.
pub async fn server_secret(db: &PgPool) -> anyhow::Result<Vec<u8>> {
    let existing: Option<Value> =
        sqlx::query_scalar("SELECT value FROM system_config WHERE key = 'secret'")
            .fetch_optional(db)
            .await?;
    if let Some(Value::String(s)) = existing {
        return Ok(hex::decode(s)?);
    }
    let secret: [u8; 32] = rand::random();
    sqlx::query("INSERT INTO system_config (key, value) VALUES ('secret', $1) ON CONFLICT DO NOTHING")
        .bind(Value::String(hex::encode(secret)))
        .execute(db)
        .await?;
    Ok(secret.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_unpatched_fields() {
        let mut base = serde_json::to_value(SystemConfig::default()).unwrap();
        merge_json(&mut base, serde_json::json!({"ml": {"faceMinFaces": 5}, "trash": {"days": 7}}));
        let cfg: SystemConfig = serde_json::from_value(base).unwrap();
        assert_eq!(cfg.ml.face_min_faces, 5);
        assert_eq!(cfg.trash.days, 7);
        assert_eq!(cfg.ml.clip_model, "ViT-B-32__openai");
        assert_eq!(cfg.image.preview_size, 1440);
    }

    #[test]
    fn old_config_without_new_sections_loads() {
        let mut base = serde_json::to_value(SystemConfig::default()).unwrap();
        merge_json(&mut base, serde_json::json!({"trash": {"days": 3}}));
        base.as_object_mut().unwrap().remove("map");
        let cfg: SystemConfig = serde_json::from_value(base).unwrap();
        assert!(cfg.map.enabled);
        assert_eq!(cfg.library.scan_cron, "0 0 * * * *");
    }
}
