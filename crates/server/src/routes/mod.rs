pub mod admin;
pub mod albums;
pub mod assets;
pub mod auth;
pub mod cast;
pub mod people;
pub mod search;
pub mod sharing;
pub mod tags;
pub mod timeline;
pub mod users;

use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::state::SharedState;

pub fn api() -> Router<SharedState> {
    Router::new()
        .merge(auth::router())
        .merge(users::router())
        .merge(admin::router())
        .merge(assets::router())
        .merge(timeline::router())
        .merge(albums::router())
        .merge(tags::router())
        .merge(people::router())
        .merge(search::router())
        .merge(sharing::router())
        .merge(cast::router())
        .fallback(|| async {
            (StatusCode::NOT_FOUND, Json(json!({"statusCode": 404, "error": "Not Found", "message": "unknown endpoint"})))
        })
}

async fn track_metrics(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "unmatched".into());
    let method = req.method().to_string();
    let res = next.run(req).await;
    let status = res.status().as_u16().to_string();
    metrics::counter!("pixelhort_http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status)
        .increment(1);
    metrics::histogram!("pixelhort_http_request_duration_seconds", "method" => method, "path" => path)
        .record(start.elapsed().as_secs_f64());
    res
}

pub fn app(state: SharedState) -> Router {
    let mut router = Router::new().nest("/api", api());
    if let Some(dir) = &state.config.web_dir {
        let index = dir.join("index.html");
        router = router.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    } else {
        router = router.fallback(|| async {
            "Pixelhort server is running. The web app was not found (set PH_WEB_DIR)."
        });
    }
    router
        .route_layer(middleware::from_fn(track_metrics))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::very_permissive())
        .with_state(state)
}

/// Prometheus exposition on the separate metrics listener.
pub fn metrics_app(state: SharedState) -> Router {
    Router::new()
        .route(
            "/metrics",
            get(|State(state): State<SharedState>| async move {
                match &state.metrics {
                    Some(h) => (StatusCode::OK, h.render()).into_response(),
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }),
        )
        .with_state(state)
}
