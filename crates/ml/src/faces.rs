//! Face detection (SCRFD, InsightFace det_10g) and recognition (ArcFace).

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Result, bail};
use image::RgbImage;
use image::imageops::{self, FilterType};
use ndarray::Array4;
use ort::session::Session;
use ort::value::Tensor;

use crate::runtime::{l2_normalize, load_session, oe, sample_bilinear};

const DET_SIZE: u32 = 640;
const NMS_THRESH: f32 = 0.4;
const STRIDES: [usize; 3] = [8, 16, 32];
const ANCHORS: usize = 2;

/// Standard ArcFace 112x112 landmark template.
const ARCFACE_DST: [[f32; 2]; 5] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [41.5493, 92.3655],
    [70.7299, 92.2041],
];

pub struct Face {
    /// x1, y1, x2, y2 in pixels of the input image.
    pub bbox: [f32; 4],
    pub score: f32,
    pub landmarks: [[f32; 2]; 5],
    pub embedding: Vec<f32>,
}

pub struct FaceModel {
    det: Mutex<Session>,
    rec: Mutex<Session>,
}

impl FaceModel {
    pub fn load(det: &Path, rec: &Path, threads: usize) -> Result<Self> {
        Ok(Self { det: Mutex::new(load_session(det, threads)?), rec: Mutex::new(load_session(rec, threads)?) })
    }

    pub fn detect_and_embed(&self, img: &RgbImage, min_score: f32) -> Result<Vec<Face>> {
        let mut faces = self.detect(img, min_score)?;
        for f in &mut faces {
            f.embedding = self.embed(img, &f.landmarks)?;
        }
        Ok(faces)
    }

    fn detect(&self, img: &RgbImage, min_score: f32) -> Result<Vec<Face>> {
        let (w, h) = img.dimensions();
        let scale = (DET_SIZE as f32 / w as f32).min(DET_SIZE as f32 / h as f32);
        let nw = ((w as f32 * scale).round() as u32).clamp(1, DET_SIZE);
        let nh = ((h as f32 * scale).round() as u32).clamp(1, DET_SIZE);
        let resized = imageops::resize(img, nw, nh, FilterType::Triangle);
        let n = DET_SIZE as usize;
        // Letterbox at the top-left corner; padding is black (0 -> (0-127.5)/128).
        let mut arr = Array4::<f32>::from_elem((1, 3, n, n), -127.5 / 128.0);
        for (x, y, p) in resized.enumerate_pixels() {
            for c in 0..3 {
                arr[[0, c, y as usize, x as usize]] = (p.0[c] as f32 - 127.5) / 128.0;
            }
        }
        let input = Tensor::from_array(arr).map_err(oe)?;

        // Collect raw outputs (shape last dim, data).
        let raw: Vec<(i64, Vec<f32>)> = {
            let mut det = self.det.lock().unwrap();
            let outputs = det.run(ort::inputs![input]).map_err(oe)?;
            let mut v = Vec::new();
            for (_, value) in outputs.iter() {
                let (shape, data) = value.try_extract_tensor::<f32>().map_err(oe)?;
                v.push((shape.last().copied().unwrap_or(1), data.to_vec()));
            }
            v
        };

        let mut cands: Vec<Face> = Vec::new();
        for stride in STRIDES {
            let fw = n / stride;
            let count = fw * fw * ANCHORS;
            let find = |dim: i64| -> Option<&Vec<f32>> {
                raw.iter().find(|(last, d)| {
                    let is_dim = match dim {
                        4 | 10 => *last == dim,
                        _ => *last != 4 && *last != 10,
                    };
                    is_dim && d.len() == count * dim as usize
                })
                .map(|(_, d)| d)
            };
            let (Some(scores), Some(boxes), Some(kps)) = (find(1), find(4), find(10)) else {
                bail!("unexpected face detector outputs (stride {stride})");
            };
            let s = stride as f32;
            for i in 0..count {
                let score = scores[i];
                if score < min_score {
                    continue;
                }
                let loc = i / ANCHORS;
                let cx = (loc % fw) as f32 * s;
                let cy = (loc / fw) as f32 * s;
                let b = &boxes[i * 4..i * 4 + 4];
                let k = &kps[i * 10..i * 10 + 10];
                let mut landmarks = [[0f32; 2]; 5];
                for (j, lm) in landmarks.iter_mut().enumerate() {
                    *lm = [(cx + k[2 * j] * s) / scale, (cy + k[2 * j + 1] * s) / scale];
                }
                cands.push(Face {
                    bbox: [
                        ((cx - b[0] * s) / scale).clamp(0.0, w as f32),
                        ((cy - b[1] * s) / scale).clamp(0.0, h as f32),
                        ((cx + b[2] * s) / scale).clamp(0.0, w as f32),
                        ((cy + b[3] * s) / scale).clamp(0.0, h as f32),
                    ],
                    score,
                    landmarks,
                    embedding: Vec::new(),
                });
            }
        }
        Ok(nms(cands, NMS_THRESH))
    }

    fn embed(&self, img: &RgbImage, landmarks: &[[f32; 2]; 5]) -> Result<Vec<f32>> {
        // Similarity transform src(landmarks) -> dst(template), least squares.
        let m = similarity(landmarks, &ARCFACE_DST);
        let (a, b, tx, ty) = (m[0], m[1], m[2], m[3]);
        let det = a * a + b * b;
        let mut arr = Array4::<f32>::zeros((1, 3, 112, 112));
        for v in 0..112usize {
            for u in 0..112usize {
                let (du, dv) = (u as f32 - tx, v as f32 - ty);
                let x = (a * du + b * dv) / det;
                let y = (-b * du + a * dv) / det;
                let p = sample_bilinear(img, x, y);
                for c in 0..3 {
                    arr[[0, c, v, u]] = (p[c] - 127.5) / 127.5;
                }
            }
        }
        let input = Tensor::from_array(arr).map_err(oe)?;
        let mut rec = self.rec.lock().unwrap();
        let outputs = rec.run(ort::inputs![input]).map_err(oe)?;
        let (_, data) = outputs[0].try_extract_tensor::<f32>().map_err(oe)?;
        let mut e = data.to_vec();
        l2_normalize(&mut e);
        Ok(e)
    }
}

/// Least-squares similarity transform (rotation, uniform scale, translation)
/// mapping `src` onto `dst`. Returns (a, b, tx, ty) for
/// `x' = a*x - b*y + tx`, `y' = b*x + a*y + ty`.
fn similarity(src: &[[f32; 2]; 5], dst: &[[f32; 2]; 5]) -> [f32; 4] {
    let n = src.len() as f32;
    let mean = |p: &[[f32; 2]; 5]| {
        let (sx, sy) = p.iter().fold((0.0, 0.0), |(x, y), q| (x + q[0], y + q[1]));
        [sx / n, sy / n]
    };
    let (ms, md) = (mean(src), mean(dst));
    let (mut sxx, mut sxy, mut norm) = (0f32, 0f32, 0f32);
    for (s, d) in src.iter().zip(dst) {
        let (sx, sy) = (s[0] - ms[0], s[1] - ms[1]);
        let (dx, dy) = (d[0] - md[0], d[1] - md[1]);
        sxx += sx * dx + sy * dy;
        sxy += sx * dy - sy * dx;
        norm += sx * sx + sy * sy;
    }
    let norm = norm.max(1e-6);
    let (a, b) = (sxx / norm, sxy / norm);
    let tx = md[0] - (a * ms[0] - b * ms[1]);
    let ty = md[1] - (b * ms[0] + a * ms[1]);
    [a, b, tx, ty]
}

fn iou(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let iw = (a[2].min(b[2]) - a[0].max(b[0]) + 1.0).max(0.0);
    let ih = (a[3].min(b[3]) - a[1].max(b[1]) + 1.0).max(0.0);
    let inter = iw * ih;
    let area = |r: &[f32; 4]| (r[2] - r[0] + 1.0) * (r[3] - r[1] + 1.0);
    inter / (area(a) + area(b) - inter)
}

fn nms(mut cands: Vec<Face>, thresh: f32) -> Vec<Face> {
    cands.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut keep: Vec<Face> = Vec::new();
    for c in cands {
        if keep.iter().all(|k| iou(&k.bbox, &c.bbox) <= thresh) {
            keep.push(c);
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_identity() {
        let m = similarity(&ARCFACE_DST, &ARCFACE_DST);
        assert!((m[0] - 1.0).abs() < 1e-4 && m[1].abs() < 1e-4 && m[2].abs() < 1e-3 && m[3].abs() < 1e-3);
    }

    #[test]
    fn similarity_scaled() {
        let src = ARCFACE_DST.map(|p| [p[0] * 2.0 + 10.0, p[1] * 2.0 + 5.0]);
        let m = similarity(&src, &ARCFACE_DST);
        assert!((m[0] - 0.5).abs() < 1e-4 && m[1].abs() < 1e-4);
        assert!((m[0] * src[0][0] + m[2] - ARCFACE_DST[0][0]).abs() < 1e-3);
    }
}
