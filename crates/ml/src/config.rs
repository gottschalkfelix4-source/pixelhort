use std::path::PathBuf;
use std::time::Duration;

const MONKT: &str = "https://huggingface.co/monkt/paddleocr-onnx/resolve/main";
/// PP-OCRv4 mobile text detector (~5 MB, ~10x faster on CPU than the PP-OCRv5 server detector).
const DEFAULT_OCR_DET: &str = "https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/ch_PP-OCRv4_det_infer.onnx";

/// Runtime configuration, read from `PH_ML_*` environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    pub listen: String,
    pub cache_dir: PathBuf,
    pub api_key: Option<String>,
    pub clip_model: String,
    pub face_model: String,
    /// Unload models that have not been used for this long. `None` = never.
    pub model_ttl: Option<Duration>,
    /// Intra-op threads per ONNX session (0 = ONNX Runtime default).
    pub threads: usize,
    /// Hugging Face base URL (for mirrors).
    pub hf_endpoint: String,
    pub ocr_det_url: String,
    pub ocr_rec_url: String,
    pub ocr_dict_url: String,
    /// Longest side of the image fed to the OCR text detector.
    pub ocr_max_side: u32,
    /// Models to load at startup (`clip`, `faces`, `ocr`, `objects`).
    pub preload: Vec<String>,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let parse_u64 = |name: &str, default: u64| -> anyhow::Result<u64> {
            match var(name) {
                Some(v) => v.parse().map_err(|_| anyhow::anyhow!("{name} must be an integer, got {v:?}")),
                None => Ok(default),
            }
        };
        let ttl = parse_u64("PH_ML_MODEL_TTL_SECS", 300)?;
        Ok(Self {
            listen: var("PH_ML_LISTEN").unwrap_or_else(|| "0.0.0.0:3003".into()),
            cache_dir: PathBuf::from(var("PH_ML_CACHE_DIR").unwrap_or_else(|| "/cache".into())),
            api_key: var("PH_ML_API_KEY"),
            clip_model: var("PH_ML_CLIP_MODEL").unwrap_or_else(|| "ViT-B-32__openai".into()),
            face_model: var("PH_ML_FACE_MODEL").unwrap_or_else(|| "buffalo_l".into()),
            model_ttl: (ttl > 0).then(|| Duration::from_secs(ttl)),
            threads: parse_u64("PH_ML_THREADS", 0)? as usize,
            hf_endpoint: var("PH_ML_HF_ENDPOINT")
                .unwrap_or_else(|| "https://huggingface.co".into())
                .trim_end_matches('/')
                .to_string(),
            ocr_det_url: var("PH_ML_OCR_DET_URL").unwrap_or_else(|| DEFAULT_OCR_DET.into()),
            ocr_rec_url: var("PH_ML_OCR_REC_URL").unwrap_or_else(|| format!("{MONKT}/languages/latin/rec.onnx")),
            ocr_dict_url: var("PH_ML_OCR_DICT_URL").unwrap_or_else(|| format!("{MONKT}/languages/latin/dict.txt")),
            ocr_max_side: parse_u64("PH_ML_OCR_MAX_SIDE", 960)?.clamp(64, 4096) as u32,
            preload: var("PH_ML_PRELOAD")
                .map(|v| v.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default(),
        })
    }

    /// URL of a file in a Hugging Face repository (main branch).
    pub fn hf_url(&self, repo: &str, path: &str) -> String {
        format!("{}/{}/resolve/main/{}", self.hf_endpoint, repo, path)
    }
}
