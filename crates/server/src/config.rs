use std::path::PathBuf;

/// Static configuration from environment variables. Runtime-editable settings
/// live in [`crate::sysconfig::SystemConfig`].
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub data_dir: PathBuf,
    pub listen: String,
    pub metrics_listen: Option<String>,
    pub web_dir: Option<PathBuf>,
    pub public_url: Option<String>,
    pub ml_url: Option<String>,
    pub ml_api_key: Option<String>,
    pub workers: bool,
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env("PH_DATABASE_URL")
            .or_else(|| env("DATABASE_URL"))
            .unwrap_or_else(|| "postgres://pixelhort:pixelhort@localhost:5432/pixelhort".into());
        let web_dir = env("PH_WEB_DIR").map(PathBuf::from).or_else(|| {
            ["/app/web", "web/dist", "../../web/dist"]
                .iter()
                .map(PathBuf::from)
                .find(|p| p.join("index.html").exists())
        });
        Ok(Self {
            database_url,
            data_dir: PathBuf::from(env("PH_DATA_DIR").unwrap_or_else(|| "./data".into())),
            listen: env("PH_LISTEN").unwrap_or_else(|| "0.0.0.0:2342".into()),
            metrics_listen: match env("PH_METRICS_LISTEN").as_deref() {
                Some("off") | Some("false") => None,
                Some(v) => Some(v.to_string()),
                None => Some("0.0.0.0:9464".into()),
            },
            web_dir,
            public_url: env("PH_PUBLIC_URL").map(|u| u.trim_end_matches('/').to_string()),
            ml_url: env("PH_ML_URL"),
            ml_api_key: env("PH_ML_API_KEY"),
            workers: env("PH_WORKERS").map(|v| v != "false" && v != "0").unwrap_or(true),
        })
    }
}
