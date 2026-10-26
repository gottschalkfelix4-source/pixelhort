//! Pixelhort ML service: CLIP embeddings, face detection/recognition, OCR and
//! zero-shot object tagging over HTTP. See `docs/API.md` ("ML service API").

mod clip;
mod config;
mod faces;
mod objects;
mod ocr;
mod runtime;

use std::collections::HashSet;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::{DefaultBodyLimit, Multipart, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use image::{ImageDecoder, ImageReader, RgbImage};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::OnceCell;

use crate::clip::{ClipTextual, ClipVisual};
use crate::config::Config;
use crate::faces::FaceModel;
use crate::ocr::OcrModel;
use crate::runtime::{Downloader, ModelSlot, blocking};

const OBJECT_MIN_SCORE: f32 = 0.1;
const OBJECT_TOP_K: usize = 5;
const MAX_UPLOAD: usize = 100 * 1024 * 1024;

struct AppState {
    cfg: Config,
    dl: Downloader,
    clip_visual: ModelSlot<ClipVisual>,
    clip_textual: ModelSlot<ClipTextual>,
    faces: ModelSlot<FaceModel>,
    ocr: ModelSlot<OcrModel>,
    label_embeddings: OnceCell<Arc<Vec<Vec<f32>>>>,
    metrics: PrometheusHandle,
}

type St = Arc<AppState>;

impl AppState {
    fn clip_repo(&self) -> String {
        format!("immich-app/{}", self.cfg.clip_model)
    }

    async fn clip_visual(&self) -> Result<Arc<ClipVisual>> {
        self.clip_visual
            .get(|| async {
                let repo = self.clip_repo();
                let model = self.dl.fetch(&self.cfg.hf_url(&repo, "visual/model.onnx")).await?;
                let pre = self.dl.fetch(&self.cfg.hf_url(&repo, "visual/preprocess_cfg.json")).await?;
                let threads = self.cfg.threads;
                blocking("load", move || ClipVisual::load(&model, &pre, threads)).await
            })
            .await
    }

    async fn clip_textual(&self) -> Result<Arc<ClipTextual>> {
        self.clip_textual
            .get(|| async {
                let repo = self.clip_repo();
                let model = self.dl.fetch(&self.cfg.hf_url(&repo, "textual/model.onnx")).await?;
                let tok = self.dl.fetch(&self.cfg.hf_url(&repo, "textual/tokenizer.json")).await?;
                let cfg = self.dl.fetch(&self.cfg.hf_url(&repo, "config.json")).await?;
                let threads = self.cfg.threads;
                blocking("load", move || ClipTextual::load(&model, &tok, &cfg, threads)).await
            })
            .await
    }

    async fn faces(&self) -> Result<Arc<FaceModel>> {
        self.faces
            .get(|| async {
                let repo = format!("immich-app/{}", self.cfg.face_model);
                let det = self.dl.fetch(&self.cfg.hf_url(&repo, "detection/model.onnx")).await?;
                let rec = self.dl.fetch(&self.cfg.hf_url(&repo, "recognition/model.onnx")).await?;
                let threads = self.cfg.threads;
                blocking("load", move || FaceModel::load(&det, &rec, threads)).await
            })
            .await
    }

    async fn ocr(&self) -> Result<Arc<OcrModel>> {
        self.ocr
            .get(|| async {
                let det = self.dl.fetch(&self.cfg.ocr_det_url).await?;
                let rec = self.dl.fetch(&self.cfg.ocr_rec_url).await?;
                let dict = self.dl.fetch(&self.cfg.ocr_dict_url).await?;
                let (threads, max_side) = (self.cfg.threads, self.cfg.ocr_max_side);
                blocking("load", move || OcrModel::load(&det, &rec, &dict, max_side, threads)).await
            })
            .await
    }

    /// Text embeddings of the object vocabulary, computed once.
    async fn label_embeddings(&self) -> Result<Arc<Vec<Vec<f32>>>> {
        self.label_embeddings
            .get_or_try_init(|| async {
                let text = self.clip_textual().await?;
                tracing::info!(labels = objects::LABELS.len(), "computing object label embeddings");
                let embs = blocking("objects_labels", move || {
                    objects::LABELS.iter().map(|l| text.embed(&format!("a photo of a {l}"))).collect::<Result<Vec<_>>>()
                })
                .await?;
                Ok::<_, anyhow::Error>(Arc::new(embs))
            })
            .await
            .cloned()
    }

    fn unload_idle(&self, ttl: Duration) {
        self.clip_visual.unload_if_idle(ttl);
        self.clip_textual.unload_if_idle(ttl);
        self.faces.unload_if_idle(ttl);
        self.ocr.unload_if_idle(ttl);
    }
}

// ---------------------------------------------------------------- errors

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: msg.into() }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("{e:#}");
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: format!("{e:#}") }
    }
}

impl From<axum::extract::multipart::MultipartError> for ApiError {
    fn from(e: axum::extract::multipart::MultipartError) -> Self {
        Self { status: e.status(), message: e.body_text() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        metrics::counter!("ph_ml_errors_total", "status" => self.status.as_u16().to_string()).increment(1);
        let body = json!({
            "statusCode": self.status.as_u16(),
            "error": self.status.canonical_reason().unwrap_or("Error"),
            "message": self.message,
        });
        (self.status, Json(body)).into_response()
    }
}

// ---------------------------------------------------------------- DTOs

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PredictResponse {
    image_width: u32,
    image_height: u32,
    clip: Option<Vec<f32>>,
    faces: Option<Vec<FaceDto>>,
    ocr: Option<OcrDto>,
    objects: Option<Vec<ObjectDto>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FaceDto {
    bounding_box: BoxDto,
    score: f32,
    embedding: Vec<f32>,
}

#[derive(Serialize)]
struct BoxDto {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

#[derive(Serialize)]
struct OcrDto {
    text: String,
    lines: Vec<OcrLineDto>,
}

#[derive(Serialize)]
struct OcrLineDto {
    text: String,
    score: f32,
    #[serde(rename = "box")]
    quad: [i32; 8],
}

#[derive(Serialize)]
struct ObjectDto {
    label: &'static str,
    score: f32,
}

#[derive(Deserialize)]
struct TextRequest {
    text: String,
}

// ---------------------------------------------------------------- handlers

async fn ping() -> &'static str {
    "pong"
}

async fn health(State(st): State<St>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "models": {"clip": true, "faces": true, "ocr": true},
        "loaded": {
            "clipVisual": st.clip_visual.is_loaded(),
            "clipTextual": st.clip_textual.is_loaded(),
            "faces": st.faces.is_loaded(),
            "ocr": st.ocr.is_loaded(),
        },
        "clipModel": st.cfg.clip_model,
        "faceModel": st.cfg.face_model,
    }))
}

async fn metrics_handler(State(st): State<St>) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], st.metrics.render())
}

async fn clip_text(State(st): State<St>, Json(req): Json<TextRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    metrics::counter!("ph_ml_requests_total", "endpoint" => "clip_text").increment(1);
    let model = st.clip_textual().await?;
    let embedding = blocking("clip_text", move || model.embed(&req.text)).await?;
    Ok(Json(json!({ "embedding": embedding })))
}

fn decode_image(bytes: &[u8]) -> Result<RgbImage> {
    let mut decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_decoder()
        .context("unsupported image format")?;
    let orientation = decoder.orientation().ok();
    let mut img = image::DynamicImage::from_decoder(decoder).context("decoding image")?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img.into_rgb8())
}

async fn predict(State(st): State<St>, mut mp: Multipart) -> Result<Json<PredictResponse>, ApiError> {
    metrics::counter!("ph_ml_requests_total", "endpoint" => "predict").increment(1);
    let mut image = None;
    let mut tasks_raw: Option<String> = None;
    let mut face_min_score = 0.7f32;
    while let Some(field) = mp.next_field().await? {
        match field.name() {
            Some("image") => image = Some(field.bytes().await?),
            Some("tasks") => tasks_raw = Some(field.text().await?),
            Some("faceMinScore") => {
                let v = field.text().await?;
                face_min_score = v
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| ApiError::bad_request(format!("invalid faceMinScore {v:?}")))?
                    .clamp(0.0, 1.0);
            }
            _ => {}
        }
    }
    let bytes = image.ok_or_else(|| ApiError::bad_request("missing multipart field 'image'"))?;
    let tasks: HashSet<String> = tasks_raw
        .as_deref()
        .unwrap_or("clip,faces,ocr,objects")
        .split(',')
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    if let Some(bad) = tasks.iter().find(|t| !["clip", "faces", "ocr", "objects"].contains(&t.as_str())) {
        return Err(ApiError::bad_request(format!("unknown task {bad:?}")));
    }
    for t in &tasks {
        metrics::counter!("ph_ml_tasks_total", "task" => t.clone()).increment(1);
    }

    let img = tokio::task::spawn_blocking(move || decode_image(&bytes))
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .map_err(|e| ApiError::bad_request(format!("{e:#}")))?;
    let (width, height) = img.dimensions();
    let img = Arc::new(img);

    let want_clip = tasks.contains("clip");
    let want_objects = tasks.contains("objects");

    let clip_fut = async {
        if !(want_clip || want_objects) {
            return Ok(None);
        }
        let model = st.clip_visual().await?;
        let img = img.clone();
        blocking("clip", move || model.embed(&img)).await.map(Some)
    };
    let faces_fut = async {
        if !tasks.contains("faces") {
            return Ok(None);
        }
        let model = st.faces().await?;
        let img = img.clone();
        blocking("faces", move || model.detect_and_embed(&img, face_min_score)).await.map(Some)
    };
    let ocr_fut = async {
        if !tasks.contains("ocr") {
            return Ok(None);
        }
        let model = st.ocr().await?;
        let img = img.clone();
        blocking("ocr", move || model.recognize(&img)).await.map(Some)
    };
    let labels_fut = async {
        if want_objects { st.label_embeddings().await.map(Some) } else { Ok::<_, anyhow::Error>(None) }
    };
    let (clip, faces, ocr, labels) = tokio::try_join!(clip_fut, faces_fut, ocr_fut, labels_fut)?;

    let objects = match (&clip, labels) {
        (Some(emb), Some(labels)) => Some(
            objects::classify(emb, &labels, OBJECT_MIN_SCORE, OBJECT_TOP_K)
                .into_iter()
                .map(|(label, score)| ObjectDto { label, score })
                .collect(),
        ),
        _ => None,
    };

    let faces = faces.map(|fs| {
        fs.into_iter()
            .map(|f| FaceDto {
                bounding_box: BoxDto {
                    x1: f.bbox[0].round() as i32,
                    y1: f.bbox[1].round() as i32,
                    x2: f.bbox[2].round() as i32,
                    y2: f.bbox[3].round() as i32,
                },
                score: f.score,
                embedding: f.embedding,
            })
            .collect()
    });

    let ocr = ocr.map(|lines| OcrDto {
        text: lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n"),
        lines: lines
            .into_iter()
            .map(|l| {
                let q = l.quad;
                let mut quad = [0i32; 8];
                for (i, p) in q.iter().enumerate() {
                    quad[2 * i] = p[0].round() as i32;
                    quad[2 * i + 1] = p[1].round() as i32;
                }
                OcrLineDto { text: l.text, score: l.score, quad }
            })
            .collect(),
    });

    Ok(Json(PredictResponse {
        image_width: width,
        image_height: height,
        clip: if want_clip { clip } else { None },
        faces,
        ocr,
        objects,
    }))
}

async fn require_key(State(st): State<St>, headers: HeaderMap, req: Request, next: Next) -> Response {
    if let Some(expected) = st.cfg.api_key.as_deref() {
        let given = headers.get("x-ml-key").and_then(|v| v.to_str().ok()).unwrap_or("");
        if !constant_time_eq(given.as_bytes(), expected.as_bytes()) {
            return ApiError { status: StatusCode::UNAUTHORIZED, message: "invalid or missing x-ml-key".into() }
                .into_response();
        }
    }
    next.run(req).await
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutting down");
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,ort=warn".into()),
        )
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();
    let cfg = Config::from_env()?;
    let metrics = PrometheusBuilder::new().install_recorder().context("installing metrics recorder")?;
    tokio::fs::create_dir_all(&cfg.cache_dir)
        .await
        .with_context(|| format!("creating cache dir {}", cfg.cache_dir.display()))?;
    tracing::info!(?cfg.listen, cache = %cfg.cache_dir.display(), clip = %cfg.clip_model, faces = %cfg.face_model,
        ttl = ?cfg.model_ttl, auth = cfg.api_key.is_some(), cuda = cfg!(feature = "cuda"),
        openvino = cfg!(feature = "openvino"), "starting pixelhort-ml");

    let state: St = Arc::new(AppState {
        dl: Downloader::new(cfg.cache_dir.clone())?,
        cfg,
        clip_visual: ModelSlot::new("clip_visual"),
        clip_textual: ModelSlot::new("clip_textual"),
        faces: ModelSlot::new("faces"),
        ocr: ModelSlot::new("ocr"),
        label_embeddings: OnceCell::new(),
        metrics,
    });

    if let Some(ttl) = state.cfg.model_ttl {
        let st = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval((ttl / 2).clamp(Duration::from_secs(1), Duration::from_secs(30)));
            loop {
                tick.tick().await;
                st.unload_idle(ttl);
            }
        });
    }

    if !state.cfg.preload.is_empty() {
        let st = state.clone();
        tokio::spawn(async move {
            for m in st.cfg.preload.clone() {
                let r = match m.as_str() {
                    "clip" => st.clip_visual().await.map(drop).and(st.clip_textual().await.map(drop)),
                    "faces" => st.faces().await.map(drop),
                    "ocr" => st.ocr().await.map(drop),
                    "objects" => st.label_embeddings().await.map(drop),
                    other => Err(anyhow::anyhow!("unknown model {other:?} in PH_ML_PRELOAD")),
                };
                if let Err(e) = r {
                    tracing::error!("preloading {m}: {e:#}");
                }
            }
        });
    }

    let protected = Router::new()
        .route("/predict", post(predict))
        .route("/clip/text", post(clip_text))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_key));
    let app = Router::new()
        .route("/ping", get(ping))
        .route("/health", get(health))
        .route("/metrics", get(metrics_handler))
        .merge(protected)
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(&state.cfg.listen)
        .await
        .with_context(|| format!("binding {}", state.cfg.listen))?;
    tracing::info!("listening on {}", state.cfg.listen);
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}
