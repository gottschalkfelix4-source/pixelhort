//! CLIP image/text embeddings (immich-app ONNX exports of OpenCLIP models).

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use image::RgbImage;
use image::imageops::{self, FilterType};
use ndarray::{Array2, Array4};
use ort::session::Session;
use ort::tensor::TensorElementType;
use ort::value::Tensor;
use serde::Deserialize;
use tokenizers::Tokenizer;

use crate::runtime::{l2_normalize, load_session, oe};

#[derive(Deserialize)]
struct PreprocessCfg {
    size: SizeCfg,
    mean: [f32; 3],
    std: [f32; 3],
}

#[derive(Deserialize)]
#[serde(untagged)]
enum SizeCfg {
    One(u32),
    Two([u32; 2]),
}

pub struct ClipVisual {
    session: Mutex<Session>,
    size: u32,
    mean: [f32; 3],
    std: [f32; 3],
}

impl ClipVisual {
    pub fn load(model: &Path, preprocess_cfg: &Path, threads: usize) -> Result<Self> {
        let cfg: PreprocessCfg = serde_json::from_slice(&std::fs::read(preprocess_cfg)?)
            .with_context(|| format!("parsing {}", preprocess_cfg.display()))?;
        let size = match cfg.size {
            SizeCfg::One(s) => s,
            SizeCfg::Two([h, _]) => h,
        };
        Ok(Self { session: Mutex::new(load_session(model, threads)?), size, mean: cfg.mean, std: cfg.std })
    }

    /// Resize shortest side to `size` (bicubic), center crop, normalize, embed.
    pub fn embed(&self, img: &RgbImage) -> Result<Vec<f32>> {
        let s = self.size;
        let (w, h) = img.dimensions();
        let scale = s as f32 / w.min(h) as f32;
        let nw = ((w as f32 * scale).round() as u32).max(s);
        let nh = ((h as f32 * scale).round() as u32).max(s);
        let resized = imageops::resize(img, nw, nh, FilterType::CatmullRom);
        let (x0, y0) = ((nw - s) / 2, (nh - s) / 2);
        let n = s as usize;
        let mut arr = Array4::<f32>::zeros((1, 3, n, n));
        for y in 0..n {
            for x in 0..n {
                let p = resized.get_pixel(x0 + x as u32, y0 + y as u32).0;
                for c in 0..3 {
                    arr[[0, c, y, x]] = (p[c] as f32 / 255.0 - self.mean[c]) / self.std[c];
                }
            }
        }
        let input = Tensor::from_array(arr).map_err(oe)?;
        let mut session = self.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input]).map_err(oe)?;
        let (_, data) = outputs[0].try_extract_tensor::<f32>().map_err(oe)?;
        let mut v = data.to_vec();
        l2_normalize(&mut v);
        Ok(v)
    }
}

pub struct ClipTextual {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    context_length: usize,
    eot_id: i64,
    int32: bool,
}

impl ClipTextual {
    pub fn load(model: &Path, tokenizer: &Path, config: &Path, threads: usize) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer).map_err(|e| anyhow!("loading tokenizer: {e}"))?;
        let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(config)?)?;
        let context_length = cfg["text_cfg"]["context_length"].as_u64().unwrap_or(77) as usize;
        let eot_id = tokenizer.token_to_id("<|endoftext|>").unwrap_or(49407) as i64;
        let session = load_session(model, threads)?;
        let int32 = session
            .inputs
            .first()
            .and_then(|i| i.input_type.tensor_type())
            .map(|t| t == TensorElementType::Int32)
            .unwrap_or(false);
        Ok(Self { session: Mutex::new(session), tokenizer, context_length, eot_id, int32 })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let enc = self.tokenizer.encode(text, true).map_err(|e| anyhow!("tokenizer: {e}"))?;
        let ctx = self.context_length;
        let mut ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
        if ids.len() > ctx {
            ids.truncate(ctx);
            ids[ctx - 1] = self.eot_id;
        }
        ids.resize(ctx, 0);
        let mut session = self.session.lock().unwrap();
        let outputs = if self.int32 {
            let arr = Array2::from_shape_vec((1, ctx), ids.iter().map(|&i| i as i32).collect())?;
            session.run(ort::inputs![Tensor::from_array(arr).map_err(oe)?]).map_err(oe)?
        } else {
            let arr = Array2::from_shape_vec((1, ctx), ids)?;
            session.run(ort::inputs![Tensor::from_array(arr).map_err(oe)?]).map_err(oe)?
        };
        let (_, data) = outputs[0].try_extract_tensor::<f32>().map_err(oe)?;
        let mut v = data.to_vec();
        l2_normalize(&mut v);
        Ok(v)
    }
}
