//! Pixelhort – self-hosted photo and video management server.

mod access;
mod auth;
mod config;
mod dlna;
mod error;
mod geocode;
mod jobs;
mod media;
mod model;
mod routes;
mod state;
mod sysconfig;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use anyhow::Context;
use metrics_exporter_prometheus::PrometheusBuilder;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::geocode::Geocoder;
use crate::jobs::JobQueue;
use crate::state::{AppState, Storage};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("PH_LOG").unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=warn")))
        .init();

    let config = Config::from_env()?;
    tracing::info!("Pixelhort {} starting", pixelhort_common::API_VERSION);

    let connect: PgConnectOptions = config.database_url.parse().context("invalid PH_DATABASE_URL")?;
    let connect = connect.options([("timezone", "UTC")]);
    let db = {
        let mut attempt = 0;
        loop {
            match PgPoolOptions::new().max_connections(30).acquire_timeout(Duration::from_secs(30)).connect_with(connect.clone()).await {
                Ok(db) => break db,
                Err(e) if attempt < 30 => {
                    attempt += 1;
                    tracing::warn!("database not reachable yet ({e}), retrying...");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
                Err(e) => return Err(e).context("could not connect to database"),
            }
        }
    };
    sqlx::migrate!("./migrations").run(&db).await.context("database migration failed")?;

    let sys = sysconfig::load(&db).await?;
    let secret = sysconfig::server_secret(&db).await?;
    let storage = Storage::new(config.data_dir.clone());
    storage.ensure_dirs().context("cannot create data directories")?;

    let metrics = PrometheusBuilder::new().install_recorder().ok();
    let http = reqwest::Client::builder()
        .user_agent(concat!("pixelhort/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .build()?;

    let jobs = JobQueue::new(db.clone());
    jobs.load_state().await?;
    jobs.recover().await?;

    for tool in ["exiftool", "ffmpeg", "ffprobe", "heif-convert", "pg_dump"] {
        if !media::command_exists(tool).await {
            tracing::warn!("external tool `{tool}` not found – related features are degraded");
        }
    }

    let state = Arc::new(AppState {
        db,
        config: config.clone(),
        sys: RwLock::new(sys),
        secret,
        http,
        jobs,
        storage,
        geocoder: Geocoder::default(),
        dlna: dlna::Dlna::default(),
        metrics,
        clip_text_cache: Mutex::new(HashMap::new()),
    });

    if state.sys().reverse_geocoding.enabled {
        let s = state.clone();
        tokio::spawn(async move {
            if let Err(e) = s.geocoder.load(&s.http, &s.storage.geodata_dir()).await {
                tracing::warn!("reverse geocoding unavailable: {e:#}");
            }
        });
    }

    {
        let s = state.clone();
        tokio::spawn(async move { routes::cast::restore_devices(&s).await });
    }

    if config.workers {
        jobs::start_workers(state.clone());
        jobs::scheduler::start(state.clone());
        jobs::library::start_watcher(state.clone());
    }

    if let Some(addr) = &config.metrics_listen {
        let listener = tokio::net::TcpListener::bind(addr).await.with_context(|| format!("cannot bind metrics on {addr}"))?;
        tracing::info!("metrics on http://{addr}/metrics");
        let app = routes::metrics_app(state.clone());
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("metrics server: {e}");
            }
        });
    }

    let listener = tokio::net::TcpListener::bind(&config.listen).await.with_context(|| format!("cannot bind {}", config.listen))?;
    tracing::info!("listening on http://{}", config.listen);
    axum::serve(listener, routes::app(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
