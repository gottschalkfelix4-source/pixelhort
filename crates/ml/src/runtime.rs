//! Model infrastructure: file download/cache, ONNX session creation and
//! lazily loaded, idle-unloadable model slots.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use tokio::io::AsyncWriteExt;

/// Converts any displayable error (e.g. `ort::Error<SessionBuilder>`) into anyhow.
pub fn oe<E: std::fmt::Display>(e: E) -> anyhow::Error {
    anyhow!("onnxruntime: {e}")
}

pub fn load_session(path: &Path, threads: usize) -> Result<Session> {
    let mut b = Session::builder()
        .map_err(oe)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(oe)?;
    if threads > 0 {
        b = b.with_intra_threads(threads).map_err(oe)?;
    }
    #[cfg(feature = "cuda")]
    {
        use ort::execution_providers::CUDAExecutionProvider;
        b = b
            .with_execution_providers([CUDAExecutionProvider::default().build()])
            .map_err(oe)?;
    }
    b.commit_from_file(path)
        .map_err(oe)
        .with_context(|| format!("loading model {}", path.display()))
}

/// Downloads files into the cache directory (once) and returns their local path.
pub struct Downloader {
    client: reqwest::Client,
    cache_dir: PathBuf,
    /// Per-URL locks so concurrent requests never fetch the same file twice.
    locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl Downloader {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("pixelhort-ml/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { client, cache_dir, locks: Mutex::new(HashMap::new()) })
    }

    /// Local cache path for a URL: `<cache>/<host>/<path...>`.
    pub fn local_path(&self, url: &str) -> Result<PathBuf> {
        let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
        let rest = rest.split(['?', '#']).next().unwrap_or(rest);
        let mut p = self.cache_dir.clone();
        for seg in rest.split('/').filter(|s| !s.is_empty()) {
            if seg == "." || seg == ".." {
                bail!("invalid model url {url}");
            }
            let clean: String = seg
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' })
                .collect();
            p.push(clean);
        }
        Ok(p)
    }

    pub async fn fetch(&self, url: &str) -> Result<PathBuf> {
        let dest = self.local_path(url)?;
        if tokio::fs::try_exists(&dest).await.unwrap_or(false) {
            return Ok(dest);
        }
        let lock = self.locks.lock().unwrap().entry(url.to_string()).or_default().clone();
        let _guard = lock.lock().await;
        if tokio::fs::try_exists(&dest).await.unwrap_or(false) {
            return Ok(dest);
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let mut tmp = dest.clone().into_os_string();
        tmp.push(".part");
        let tmp = PathBuf::from(tmp);
        tracing::info!(%url, dest = %dest.display(), "downloading model file");
        let start = Instant::now();
        let mut resp = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?
            .error_for_status()
            .with_context(|| format!("GET {url}"))?;
        let mut file = tokio::fs::File::create(&tmp)
            .await
            .with_context(|| format!("creating {}", tmp.display()))?;
        let mut bytes = 0u64;
        while let Some(chunk) = resp.chunk().await.with_context(|| format!("reading {url}"))? {
            file.write_all(&chunk).await?;
            bytes += chunk.len() as u64;
        }
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        tokio::fs::rename(&tmp, &dest).await?;
        metrics::counter!("ph_ml_download_bytes_total").increment(bytes);
        tracing::info!(%url, bytes, secs = start.elapsed().as_secs_f32(), "download complete");
        Ok(dest)
    }
}

/// A lazily loaded model. Loading is guarded by an async mutex, so concurrent
/// requests wait for one load instead of downloading/loading twice.
pub struct ModelSlot<T> {
    name: &'static str,
    inner: tokio::sync::Mutex<Option<Arc<T>>>,
    last_used: Mutex<Instant>,
    loaded: AtomicBool,
}

impl<T: Send + Sync + 'static> ModelSlot<T> {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            inner: tokio::sync::Mutex::new(None),
            last_used: Mutex::new(Instant::now()),
            loaded: AtomicBool::new(false),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::Relaxed)
    }

    fn touch(&self) {
        *self.last_used.lock().unwrap() = Instant::now();
    }

    pub async fn get<F, Fut>(&self, load: F) -> Result<Arc<T>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        self.touch();
        let mut guard = self.inner.lock().await;
        if let Some(m) = guard.as_ref() {
            return Ok(m.clone());
        }
        tracing::info!(model = self.name, "loading model");
        let start = Instant::now();
        let model = Arc::new(load().await.with_context(|| format!("loading {} model", self.name))?);
        *guard = Some(model.clone());
        self.loaded.store(true, Ordering::Relaxed);
        self.touch();
        metrics::gauge!("ph_ml_model_loaded", "model" => self.name).set(1.0);
        tracing::info!(model = self.name, secs = start.elapsed().as_secs_f32(), "model loaded");
        Ok(model)
    }

    /// Drops the model if it has been idle for at least `ttl`. In-flight
    /// requests keep their own `Arc` and finish normally.
    pub fn unload_if_idle(&self, ttl: Duration) {
        if !self.is_loaded() || self.last_used.lock().unwrap().elapsed() < ttl {
            return;
        }
        if let Ok(mut guard) = self.inner.try_lock() {
            if guard.take().is_some() {
                self.loaded.store(false, Ordering::Relaxed);
                metrics::gauge!("ph_ml_model_loaded", "model" => self.name).set(0.0);
                tracing::info!(model = self.name, "unloaded idle model");
            }
        }
    }
}

/// Runs CPU-heavy work on the blocking pool and records its duration.
pub async fn blocking<T, F>(task: &'static str, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let start = Instant::now();
    let r = tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| anyhow!("{task} worker failed: {e}"))?;
    metrics::histogram!("ph_ml_inference_seconds", "task" => task).record(start.elapsed().as_secs_f64());
    r
}

pub fn l2_normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

/// Bilinear sample of an RGB image; out-of-bounds pixels are black.
#[inline]
pub fn sample_bilinear(img: &image::RgbImage, x: f32, y: f32) -> [f32; 3] {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;
    let (x0, y0) = (x0 as i64, y0 as i64);
    let px = |xx: i64, yy: i64| -> [f32; 3] {
        if xx < 0 || yy < 0 || xx >= w || yy >= h {
            [0.0; 3]
        } else {
            let p = img.get_pixel(xx as u32, yy as u32).0;
            [p[0] as f32, p[1] as f32, p[2] as f32]
        }
    };
    let a = px(x0, y0);
    let b = px(x0 + 1, y0);
    let c = px(x0, y0 + 1);
    let d = px(x0 + 1, y0 + 1);
    let mut out = [0.0; 3];
    for i in 0..3 {
        out[i] = a[i] * (1.0 - fx) * (1.0 - fy) + b[i] * fx * (1.0 - fy) + c[i] * (1.0 - fx) * fy + d[i] * fx * fy;
    }
    out
}
