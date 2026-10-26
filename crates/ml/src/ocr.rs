//! OCR with PaddleOCR (PP-OCRv5) ONNX models: DB text detection + CTC recognition.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use image::RgbImage;
use image::imageops::{self, FilterType};
use ndarray::Array4;
use ort::session::Session;
use ort::value::Tensor;

use crate::runtime::{load_session, oe, sample_bilinear};

// DB post-processing parameters (PaddleOCR defaults).
const BIN_THRESH: f32 = 0.3;
const BOX_THRESH: f32 = 0.6;
const UNCLIP_RATIO: f32 = 1.5;
const MIN_SIZE: f32 = 3.0;
/// Recognized lines below this confidence are dropped.
const DROP_SCORE: f32 = 0.5;
const REC_MIN_WIDTH: u32 = 320;
const REC_MAX_WIDTH: u32 = 3200;
const DET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const DET_STD: [f32; 3] = [0.229, 0.224, 0.225];

pub struct OcrLine {
    pub text: String,
    pub score: f32,
    /// tl, tr, br, bl corners in pixels of the input image.
    pub quad: [[f32; 2]; 4],
}

pub struct OcrModel {
    det: Mutex<Session>,
    rec: Mutex<Session>,
    /// Index 0 = CTC blank; then the dictionary; optionally a trailing space.
    charset: Vec<String>,
    rec_height: u32,
    max_side: u32,
}

impl OcrModel {
    pub fn load(det: &Path, rec: &Path, dict: &Path, max_side: u32, threads: usize) -> Result<Self> {
        let dict_text = std::fs::read_to_string(dict).with_context(|| format!("reading {}", dict.display()))?;
        let mut charset = vec![String::new()];
        charset.extend(
            dict_text
                .split('\n')
                .map(|l| l.trim_end_matches('\r').to_string())
                .filter(|l| !l.is_empty()),
        );
        let rec = load_session(rec, threads)?;
        // Fixed input height if the model declares one, else PP-OCRv5's 48.
        let rec_height = rec
            .inputs
            .first()
            .and_then(|i| i.input_type.tensor_shape().map(|s| s.to_vec()))
            .and_then(|s| s.get(2).copied())
            .filter(|&h| h > 0)
            .unwrap_or(48) as u32;
        Ok(Self {
            det: Mutex::new(load_session(det, threads)?),
            rec: Mutex::new(rec),
            charset,
            rec_height,
            max_side,
        })
    }

    pub fn recognize(&self, img: &RgbImage) -> Result<Vec<OcrLine>> {
        let quads = self.detect(img)?;
        let mut lines = Vec::new();
        for quad in quads {
            let Some(crop) = crop_quad(img, &quad) else { continue };
            let (text, score) = self.read_line(&crop)?;
            let text = text.trim().to_string();
            if text.is_empty() || score < DROP_SCORE {
                continue;
            }
            lines.push(OcrLine { text, score, quad });
        }
        Ok(lines)
    }

    fn detect(&self, img: &RgbImage) -> Result<Vec<[[f32; 2]; 4]>> {
        let (w, h) = img.dimensions();
        let scale = (self.max_side as f32 / w.max(h) as f32).min(1.0);
        let round32 = |v: f32| (((v / 32.0).round() as u32) * 32).max(32);
        let (rw, rh) = (round32(w as f32 * scale), round32(h as f32 * scale));
        let resized = imageops::resize(img, rw, rh, FilterType::Triangle);
        let (rwu, rhu) = (rw as usize, rh as usize);
        let mut arr = Array4::<f32>::zeros((1, 3, rhu, rwu));
        for (x, y, p) in resized.enumerate_pixels() {
            // Paddle models expect BGR channel order.
            for c in 0..3 {
                let v = p.0[2 - c] as f32 / 255.0;
                arr[[0, c, y as usize, x as usize]] = (v - DET_MEAN[c]) / DET_STD[c];
            }
        }
        let input = Tensor::from_array(arr).map_err(oe)?;
        let (mw, mh, prob) = {
            let mut det = self.det.lock().unwrap();
            let outputs = det.run(ort::inputs![input]).map_err(oe)?;
            let (shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(oe)?;
            let n = shape.len();
            (shape[n - 1] as usize, shape[n - 2] as usize, data.to_vec())
        };
        let (sx, sy) = (w as f32 / mw as f32, h as f32 / mh as f32);
        let mut quads: Vec<[[f32; 2]; 4]> = db_boxes(&prob, mw, mh)
            .into_iter()
            .map(|q| q.map(|p| [(p[0] * sx).clamp(0.0, w as f32 - 1.0), (p[1] * sy).clamp(0.0, h as f32 - 1.0)]))
            .collect();
        sort_reading_order(&mut quads);
        Ok(quads)
    }

    fn read_line(&self, crop: &RgbImage) -> Result<(String, f32)> {
        let hgt = self.rec_height;
        let ratio = crop.width() as f32 / crop.height().max(1) as f32;
        let rw = ((hgt as f32 * ratio).ceil() as u32).clamp(1, REC_MAX_WIDTH);
        let padded_w = rw.max(REC_MIN_WIDTH);
        let resized = imageops::resize(crop, rw, hgt, FilterType::Triangle);
        let mut arr = Array4::<f32>::zeros((1, 3, hgt as usize, padded_w as usize));
        for (x, y, p) in resized.enumerate_pixels() {
            for c in 0..3 {
                arr[[0, c, y as usize, x as usize]] = (p.0[2 - c] as f32 / 255.0 - 0.5) / 0.5;
            }
        }
        let input = Tensor::from_array(arr).map_err(oe)?;
        let mut rec = self.rec.lock().unwrap();
        let outputs = rec.run(ort::inputs![input]).map_err(oe)?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(oe)?;
        let classes = shape[shape.len() - 1] as usize;
        let steps = data.len() / classes.max(1);
        let space_idx = self.charset.len(); // dictionary + blank; next index is the space
        let mut text = String::new();
        let (mut sum, mut cnt) = (0f32, 0usize);
        let mut prev = usize::MAX;
        for t in 0..steps {
            let row = &data[t * classes..(t + 1) * classes];
            let (idx, p) = row
                .iter()
                .enumerate()
                .fold((0, f32::MIN), |acc, (i, &v)| if v > acc.1 { (i, v) } else { acc });
            if idx != 0 && idx != prev {
                if let Some(ch) = self.charset.get(idx) {
                    text.push_str(ch);
                } else if idx == space_idx {
                    text.push(' ');
                }
                sum += p;
                cnt += 1;
            }
            prev = idx;
        }
        Ok((text, if cnt > 0 { sum / cnt as f32 } else { 0.0 }))
    }
}

/// DB-net post-processing: binarize, connected components, min-area rectangles,
/// score filtering and unclip expansion. Returns quads in probability-map pixels.
fn db_boxes(prob: &[f32], w: usize, h: usize) -> Vec<[[f32; 2]; 4]> {
    let mut label = vec![0u32; w * h];
    let mut out = Vec::new();
    let mut next = 0u32;
    let mut stack = Vec::new();
    for start in 0..w * h {
        if label[start] != 0 || prob[start] <= BIN_THRESH {
            continue;
        }
        next += 1;
        // Flood fill (8-connectivity), tracking per-row x extents and score.
        let mut rows: std::collections::BTreeMap<usize, (usize, usize)> = Default::default();
        let (mut sum, mut cnt) = (0f32, 0usize);
        label[start] = next;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            sum += prob[i];
            cnt += 1;
            let e = rows.entry(y).or_insert((x, x));
            e.0 = e.0.min(x);
            e.1 = e.1.max(x);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if label[j] == 0 && prob[j] > BIN_THRESH {
                        label[j] = next;
                        stack.push(j);
                    }
                }
            }
        }
        if cnt < 4 || sum / (cnt as f32) < BOX_THRESH {
            continue;
        }
        let mut pts: Vec<[f32; 2]> = Vec::with_capacity(rows.len() * 2);
        for (&y, &(x0, x1)) in &rows {
            pts.push([x0 as f32, y as f32]);
            pts.push([x1 as f32, y as f32]);
        }
        let hull = convex_hull(pts);
        let Some((center, (rw, rh), angle)) = min_area_rect(&hull) else { continue };
        if rw.min(rh) < MIN_SIZE {
            continue;
        }
        let area = rw * rh;
        let perimeter = 2.0 * (rw + rh);
        let d = area * UNCLIP_RATIO / perimeter;
        let (rw, rh) = (rw + 2.0 * d, rh + 2.0 * d);
        if rw.min(rh) < MIN_SIZE + 2.0 {
            continue;
        }
        out.push(order_quad(rect_corners(center, (rw, rh), angle)));
    }
    out
}

fn cross(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}

/// Andrew's monotone chain.
fn convex_hull(mut pts: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let mut lower: Vec<[f32; 2]> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<[f32; 2]> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Rotating-calipers minimum-area rectangle of a convex hull.
/// Returns (center, (width, height), angle) where width runs along `angle`.
fn min_area_rect(hull: &[[f32; 2]]) -> Option<([f32; 2], (f32, f32), f32)> {
    if hull.is_empty() {
        return None;
    }
    if hull.len() < 3 {
        // Degenerate (line or point): treat as axis-aligned.
        let (x0, x1) = hull.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p[0]), a.1.max(p[0])));
        let (y0, y1) = hull.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
        return Some(([(x0 + x1) / 2.0, (y0 + y1) / 2.0], (x1 - x0 + 1.0, y1 - y0 + 1.0), 0.0));
    }
    let mut best: Option<(f32, [f32; 2], (f32, f32), f32)> = None;
    for i in 0..hull.len() {
        let a = hull[i];
        let b = hull[(i + 1) % hull.len()];
        let angle = (b[1] - a[1]).atan2(b[0] - a[0]);
        let (c, s) = (angle.cos(), angle.sin());
        let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in hull {
            let u = p[0] * c + p[1] * s;
            let v = -p[0] * s + p[1] * c;
            u0 = u0.min(u);
            u1 = u1.max(u);
            v0 = v0.min(v);
            v1 = v1.max(v);
        }
        // +1: pixel centers -> pixel extents.
        let (rw, rh) = (u1 - u0 + 1.0, v1 - v0 + 1.0);
        let area = rw * rh;
        if best.as_ref().is_none_or(|b| area < b.0) {
            let (cu, cv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0);
            let center = [cu * c - cv * s, cu * s + cv * c];
            best = Some((area, center, (rw, rh), angle));
        }
    }
    best.map(|(_, c, wh, a)| (c, wh, a))
}

fn rect_corners(center: [f32; 2], (w, h): (f32, f32), angle: f32) -> [[f32; 2]; 4] {
    let (c, s) = (angle.cos(), angle.sin());
    let (hw, hh) = (w / 2.0, h / 2.0);
    [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)].map(|(u, v)| [center[0] + u * c - v * s, center[1] + u * s + v * c])
}

/// Orders corners as top-left, top-right, bottom-right, bottom-left (PaddleOCR get_mini_boxes).
fn order_quad(mut p: [[f32; 2]; 4]) -> [[f32; 2]; 4] {
    p.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let (l0, l1) = if p[0][1] <= p[1][1] { (p[0], p[1]) } else { (p[1], p[0]) };
    let (r0, r1) = if p[2][1] <= p[3][1] { (p[2], p[3]) } else { (p[3], p[2]) };
    [l0, r0, r1, l1]
}

fn sort_reading_order(q: &mut [[[f32; 2]; 4]]) {
    q.sort_by(|a, b| a[0][1].total_cmp(&b[0][1]).then(a[0][0].total_cmp(&b[0][0])));
    // Boxes on (roughly) the same line are ordered left to right.
    for i in 0..q.len() {
        let mut j = i;
        while j > 0 && (q[j][0][1] - q[j - 1][0][1]).abs() < 10.0 && q[j][0][0] < q[j - 1][0][0] {
            q.swap(j, j - 1);
            j -= 1;
        }
    }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Rectifies a (rotated) text quad into an upright crop.
fn crop_quad(img: &RgbImage, q: &[[f32; 2]; 4]) -> Option<RgbImage> {
    let cw = dist(q[0], q[1]).max(dist(q[3], q[2])).round() as u32;
    let ch = dist(q[0], q[3]).max(dist(q[1], q[2])).round() as u32;
    if cw < 2 || ch < 2 {
        return None;
    }
    let (o, ex, ey) = (q[0], q[1], q[3]);
    let mut out = RgbImage::new(cw, ch);
    for y in 0..ch {
        let fy = (y as f32 + 0.5) / ch as f32;
        for x in 0..cw {
            let fx = (x as f32 + 0.5) / cw as f32;
            let sx = o[0] + (ex[0] - o[0]) * fx + (ey[0] - o[0]) * fy - 0.5;
            let sy = o[1] + (ex[1] - o[1]) * fx + (ey[1] - o[1]) * fy - 0.5;
            let p = sample_bilinear(img, sx, sy);
            out.put_pixel(x, y, image::Rgb(p.map(|v| v.round().clamp(0.0, 255.0) as u8)));
        }
    }
    // Vertical text: rotate 90° counter-clockwise like PaddleOCR (np.rot90).
    if ch as f32 / cw as f32 >= 1.5 {
        out = imageops::rotate270(&out);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rectangle_blob() {
        let (w, h) = (100, 50);
        let mut prob = vec![0f32; w * h];
        for y in 20..30 {
            for x in 10..80 {
                prob[y * w + x] = 0.9;
            }
        }
        let boxes = db_boxes(&prob, w, h);
        assert_eq!(boxes.len(), 1);
        let q = boxes[0];
        // unclipped: 70x10 -> d = 700*1.5/160 ≈ 6.56
        assert!((q[0][0] - 3.4).abs() < 1.0, "{q:?}");
        assert!((q[2][0] - 86.0).abs() < 1.5, "{q:?}");
        assert!(q[0][1] < 20.0 && q[2][1] > 29.0, "{q:?}");
    }
}
