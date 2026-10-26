//! Image/video helpers: decoding (incl. HEIC/RAW/video frames via external tools),
//! orientation, non-destructive edits, encoding and thumbhash.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, anyhow, bail};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbImage};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Edits {
    pub crop: Option<Crop>,
    pub rotate: i32,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub filter: Option<String>,
}

impl Edits {
    pub fn validate(&self) -> Result<(), String> {
        if ![0, 90, 180, 270].contains(&self.rotate.rem_euclid(360)) {
            return Err("rotate must be 0, 90, 180 or 270".into());
        }
        for (n, v) in [("brightness", self.brightness), ("contrast", self.contrast), ("saturation", self.saturation)] {
            if !(-100.0..=100.0).contains(&v) {
                return Err(format!("{n} must be between -100 and 100"));
            }
        }
        if let Some(c) = &self.crop {
            let ok = c.x >= 0.0 && c.y >= 0.0 && c.width > 0.0 && c.height > 0.0
                && c.x + c.width <= 1.0001 && c.y + c.height <= 1.0001;
            if !ok {
                return Err("crop must be normalized to 0..1".into());
            }
        }
        if let Some(f) = &self.filter {
            if !["bw", "sepia", "warm", "cool", "vintage"].contains(&f.as_str()) {
                return Err("unknown filter".into());
            }
        }
        Ok(())
    }

    pub fn is_identity(&self) -> bool {
        *self == Edits::default()
    }
}

const NATIVE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "jpe", "png", "gif", "webp", "tif", "tiff", "bmp"];
const HEIF_EXTENSIONS: &[&str] = &["heic", "heif", "hif", "avif"];

pub fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub async fn command_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg(if cmd == "exiftool" { "-ver" } else { "-version" })
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok()
}

async fn run(cmd: &str, args: &[&std::ffi::OsStr]) -> anyhow::Result<Vec<u8>> {
    let out = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .with_context(|| format!("failed to start {cmd}"))?;
    if !out.status.success() {
        bail!("{cmd} failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(out.stdout)
}

macro_rules! os_args {
    ($($a:expr),* $(,)?) => { &[$(std::ffi::OsStr::new($a)),*] };
}

/// Reads all metadata tags with exiftool (`-n` numeric values).
pub async fn exiftool(path: &Path) -> anyhow::Result<Map<String, Value>> {
    let out = run(
        "exiftool",
        os_args!["-json", "-n", "-api", "largefilesupport=1", path.as_os_str()],
    )
    .await?;
    let mut arr: Vec<Map<String, Value>> = serde_json::from_slice(&out)?;
    arr.pop().ok_or_else(|| anyhow!("exiftool returned nothing"))
}

/// Extracts a binary tag (e.g. embedded RAW preview) with exiftool.
pub async fn exiftool_binary(path: &Path, tag: &str) -> Option<Vec<u8>> {
    let arg = format!("-{tag}");
    let out = run("exiftool", os_args!["-b", arg.as_str(), path.as_os_str()]).await.ok()?;
    (out.len() > 1024).then_some(out)
}

#[derive(Debug, Default, Clone)]
pub struct Probe {
    pub duration: Option<f64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// Clockwise degrees to display the video upright (0, 90, 180 or 270).
    pub rotation: i32,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub format: Option<String>,
}

pub async fn ffprobe(path: &Path) -> anyhow::Result<Probe> {
    let out = run(
        "ffprobe",
        os_args!["-v", "error", "-print_format", "json", "-show_format", "-show_streams", path.as_os_str()],
    )
    .await?;
    let v: Value = serde_json::from_slice(&out)?;
    let mut p = Probe {
        duration: v["format"]["duration"].as_str().and_then(|d| d.parse().ok()),
        format: v["format"]["format_name"].as_str().map(String::from),
        ..Default::default()
    };
    for s in v["streams"].as_array().into_iter().flatten() {
        match s["codec_type"].as_str() {
            Some("video") if p.video_codec.is_none() => {
                p.video_codec = s["codec_name"].as_str().map(String::from);
                p.width = s["width"].as_i64().map(|w| w as i32);
                p.height = s["height"].as_i64().map(|h| h as i32);
                // The display matrix rotation counts counter-clockwise, the old `rotate` tag clockwise.
                let rot = s["tags"]["rotate"].as_str().and_then(|r| r.parse::<i32>().ok()).or_else(|| {
                    s["side_data_list"].as_array().and_then(|l| {
                        l.iter().find_map(|d| d["rotation"].as_i64().map(|r| -r as i32))
                    })
                });
                p.rotation = rot.unwrap_or(0).rem_euclid(360);
            }
            Some("audio") if p.audio_codec.is_none() => {
                p.audio_codec = s["codec_name"].as_str().map(String::from);
            }
            _ => {}
        }
    }
    Ok(p)
}

/// Grabs a representative frame from a video as an image (ffmpeg auto-rotates). With `gpu`
/// (a VAAPI render node) the video is decoded on the GPU, falling back to software.
pub async fn video_frame(path: &Path, duration: Option<f64>, gpu: Option<&str>) -> anyhow::Result<DynamicImage> {
    let seek = duration.map(|d| (d / 2.0).min(1.0)).unwrap_or(0.0);
    if let Some(dev) = gpu {
        match grab_frame(path, seek, Some(dev)).await {
            Ok(img) => return Ok(img),
            Err(e) => tracing::debug!(file = %path.display(), "GPU frame decoding failed: {e:#}"),
        }
    }
    grab_frame(path, seek, None).await
}

async fn grab_frame(path: &Path, seek: f64, gpu: Option<&str>) -> anyhow::Result<DynamicImage> {
    let seek = format!("{seek:.3}");
    for ss in [Some(seek.as_str()), None] {
        let mut args = os_args!["-hide_banner", "-loglevel", "error"].to_vec();
        if let Some(dev) = gpu {
            args.extend(os_args!["-hwaccel", "vaapi", "-hwaccel_device", dev]);
        }
        if let Some(ss) = ss {
            args.extend(os_args!["-ss", ss]);
        }
        args.extend(os_args!["-i", path.as_os_str(), "-frames:v", "1", "-f", "image2pipe", "-vcodec", "mjpeg", "-q:v", "2", "-"]);
        let bytes = run("ffmpeg", &args).await?;
        // Very short clips yield nothing after the seek point: retry from the start.
        if !bytes.is_empty() {
            return decode_bytes(bytes).await;
        }
    }
    bail!("ffmpeg returned no frame")
}

async fn decode_bytes(bytes: Vec<u8>) -> anyhow::Result<DynamicImage> {
    tokio::task::spawn_blocking(move || {
        image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?.decode().map_err(Into::into)
    })
    .await?
}

async fn decode_file(path: PathBuf) -> anyhow::Result<DynamicImage> {
    tokio::task::spawn_blocking(move || {
        let mut reader = image::ImageReader::open(&path)?.with_guessed_format()?;
        reader.no_limits();
        reader.decode().map_err(Into::into)
    })
    .await?
}

fn is_missing_tool(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>().is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

async fn heif_convert(path: &Path) -> anyhow::Result<DynamicImage> {
    let tmp = std::env::temp_dir().join(format!("ph-{}.jpg", uuid::Uuid::new_v4()));
    // libheif ≥ 1.17 ships the tool as `heif-dec`; older releases call it `heif-convert`.
    let mut res = run("heif-convert", os_args!["-q", "92", path.as_os_str(), tmp.as_os_str()]).await;
    if res.as_ref().is_err_and(is_missing_tool) {
        res = run("heif-dec", os_args!["-q", "92", path.as_os_str(), "-o", tmp.as_os_str()]).await;
    }
    let img = match res {
        Ok(_) => decode_file(tmp.clone()).await,
        Err(e) => Err(e),
    };
    let _ = tokio::fs::remove_file(&tmp).await;
    img
}

async fn ffmpeg_decode(path: &Path) -> anyhow::Result<DynamicImage> {
    let bytes = run(
        "ffmpeg",
        os_args![
            "-hide_banner", "-loglevel", "error", "-i", path.as_os_str(), "-frames:v", "1",
            "-f", "image2pipe", "-vcodec", "png", "-"
        ],
    )
    .await?;
    decode_bytes(bytes).await
}

/// Decodes any supported still image or a video frame and returns it upright.
pub async fn load_upright(
    path: &Path,
    is_video: bool,
    orientation: Option<i32>,
    duration: Option<f64>,
    gpu: Option<&str>,
) -> anyhow::Result<DynamicImage> {
    if is_video {
        return video_frame(path, duration, gpu).await;
    }
    let ext = extension(path);
    let raw = pixelhort_common::RAW_EXTENSIONS.contains(&ext.as_str());
    let orient = |img: DynamicImage| apply_orientation(img, orientation.unwrap_or(1));

    if NATIVE_EXTENSIONS.contains(&ext.as_str()) && !raw {
        if let Ok(img) = decode_file(path.to_path_buf()).await {
            return Ok(orient(img));
        }
    }
    let mut heif_err = None;
    if HEIF_EXTENSIONS.contains(&ext.as_str()) {
        // libheif applies the HEIF transformations itself.
        match heif_convert(path).await {
            Ok(img) => return Ok(img),
            Err(e) => heif_err = Some(e),
        }
    }
    if raw || !NATIVE_EXTENSIONS.contains(&ext.as_str()) {
        for tag in ["JpgFromRaw", "PreviewImage", "OtherImage"] {
            if let Some(bytes) = exiftool_binary(path, tag).await {
                if let Ok(img) = decode_bytes(bytes).await {
                    if img.width() >= 640 || img.height() >= 640 {
                        return Ok(orient(img));
                    }
                }
            }
        }
    }
    if let Ok(img) = decode_file(path.to_path_buf()).await {
        return Ok(orient(img));
    }
    let res = ffmpeg_decode(path).await.map(orient);
    // For HEIF the libheif error is the telling one (ffmpeg < 7 cannot read HEIF at all).
    let res = match heif_err {
        Some(h) => res.map_err(|e| anyhow!("{h:#}; {e:#}")),
        None => res,
    };
    res.context("unsupported image format")
}

pub fn apply_orientation(img: DynamicImage, orientation: i32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

fn clamp(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Applies non-destructive edits (rotate → flip → crop → color).
pub fn apply_edits(img: DynamicImage, e: &Edits) -> DynamicImage {
    let mut img = match e.rotate.rem_euclid(360) {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    };
    if e.flip_horizontal {
        img = img.fliph();
    }
    if e.flip_vertical {
        img = img.flipv();
    }
    if let Some(c) = &e.crop {
        let (w, h) = img.dimensions();
        let x = (c.x.clamp(0.0, 1.0) * w as f32) as u32;
        let y = (c.y.clamp(0.0, 1.0) * h as f32) as u32;
        let cw = ((c.width * w as f32) as u32).clamp(1, w.saturating_sub(x).max(1));
        let ch = ((c.height * h as f32) as u32).clamp(1, h.saturating_sub(y).max(1));
        img = img.crop_imm(x, y, cw, ch);
    }
    let needs_color = e.brightness != 0.0 || e.contrast != 0.0 || e.saturation != 0.0 || e.filter.is_some();
    if !needs_color {
        return img;
    }
    let mut rgb: RgbImage = img.to_rgb8();
    let b = e.brightness * 2.55;
    let c = e.contrast * 2.55;
    let cf = (259.0 * (c + 255.0)) / (255.0 * (259.0 - c));
    let s = 1.0 + e.saturation / 100.0;
    let filter = e.filter.as_deref();
    for p in rgb.pixels_mut() {
        let [mut r, mut g, mut bl] = [p[0] as f32, p[1] as f32, p[2] as f32];
        // brightness + contrast
        r = cf * (r + b - 128.0) + 128.0;
        g = cf * (g + b - 128.0) + 128.0;
        bl = cf * (bl + b - 128.0) + 128.0;
        // saturation
        let l = 0.299 * r + 0.587 * g + 0.114 * bl;
        r = l + (r - l) * s;
        g = l + (g - l) * s;
        bl = l + (bl - l) * s;
        match filter {
            Some("bw") => {
                let l = 0.299 * r + 0.587 * g + 0.114 * bl;
                (r, g, bl) = (l, l, l);
            }
            Some("sepia") | Some("vintage") => {
                let (sr, sg, sb) = (
                    0.393 * r + 0.769 * g + 0.189 * bl,
                    0.349 * r + 0.686 * g + 0.168 * bl,
                    0.272 * r + 0.534 * g + 0.131 * bl,
                );
                if filter == Some("vintage") {
                    // half-strength sepia, lifted blacks, lower contrast
                    r = (r + sr) / 2.0 * 0.85 + 30.0;
                    g = (g + sg) / 2.0 * 0.85 + 25.0;
                    bl = (bl + sb) / 2.0 * 0.85 + 20.0;
                } else {
                    (r, g, bl) = (sr, sg, sb);
                }
            }
            Some("warm") => {
                r += 18.0;
                bl -= 18.0;
            }
            Some("cool") => {
                r -= 18.0;
                bl += 18.0;
            }
            _ => {}
        }
        *p = image::Rgb([clamp(r), clamp(g), clamp(bl)]);
    }
    DynamicImage::ImageRgb8(rgb)
}

/// Scales down so the longest edge is at most `max`.
pub fn fit_longest(img: &DynamicImage, max: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w.max(h) <= max {
        return img.clone();
    }
    // Lanczos on a full 12–48 MP photo is slow: shrink with a fast box filter first.
    if w.max(h) > max * 3 {
        return img.thumbnail(max * 2, max * 2).resize(max, max, FilterType::Lanczos3);
    }
    img.resize(max, max, FilterType::Lanczos3)
}

/// Scales down so the shortest edge is `min` (keeps aspect ratio, caps extreme panoramas).
pub fn fit_shortest(img: &DynamicImage, min: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let short = w.min(h);
    if short <= min {
        return img.clone();
    }
    let scale = min as f32 / short as f32;
    let (nw, nh) = (((w as f32 * scale) as u32).clamp(1, min * 4), ((h as f32 * scale) as u32).clamp(1, min * 4));
    img.resize_exact(nw, nh, FilterType::Triangle)
}

pub fn encode_jpeg(img: &DynamicImage, quality: u8) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let rgb = img.to_rgb8();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
    enc.encode_image(&rgb)?;
    Ok(buf)
}

pub fn thumbhash(img: &DynamicImage) -> Vec<u8> {
    let small = img.resize(100, 100, FilterType::Triangle).to_rgba8();
    thumbhash::rgba_to_thumb_hash(small.width() as usize, small.height() as usize, small.as_raw())
}

/// Finds the embedded MP4 of a Google/Samsung motion photo. Returns the byte offset.
pub fn find_embedded_video(bytes: &[u8], micro_video_offset: Option<usize>) -> Option<usize> {
    if let Some(off) = micro_video_offset {
        if off > 0 && off < bytes.len() {
            let start = bytes.len() - off;
            if bytes.get(start + 4..start + 8) == Some(b"ftyp") {
                return Some(start);
            }
        }
    }
    // Scan for an ISO-BMFF `ftyp` box after the JPEG data.
    let mut i = 1024.min(bytes.len());
    while let Some(pos) = memfind(&bytes[i..], b"ftyp") {
        let at = i + pos;
        if at >= 4 {
            let size = u32::from_be_bytes([bytes[at - 4], bytes[at - 3], bytes[at - 2], bytes[at - 1]]);
            let brand = bytes.get(at + 4..at + 8).unwrap_or(&[]);
            if (8..=256).contains(&size) && brand.iter().all(|b| b.is_ascii_alphanumeric() || *b == b' ') {
                return Some(at - 4);
            }
        }
        i = at + 4;
    }
    None
}

fn memfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

pub fn mime_for(path: &Path) -> String {
    match extension(path).as_str() {
        "heic" | "hif" => "image/heic".into(),
        "heif" => "image/heif".into(),
        "dng" => "image/x-adobe-dng".into(),
        "insp" => "image/jpeg".into(),
        "insv" => "video/mp4".into(),
        "mts" | "m2ts" => "video/mp2t".into(),
        "raw" | "cr2" | "cr3" | "nef" | "arw" | "raf" | "orf" | "rw2" | "pef" | "srw" => {
            format!("image/x-{}", extension(path))
        }
        _ => mime_guess::from_path(path).first_or_octet_stream().to_string(),
    }
}

pub fn format_exposure(seconds: f64) -> String {
    if seconds >= 1.0 || seconds <= 0.0 {
        format!("{seconds}")
    } else {
        format!("1/{}", (1.0 / seconds).round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_rotate_and_crop() {
        let img = DynamicImage::new_rgb8(400, 200);
        let e = Edits {
            rotate: 90,
            crop: Some(Crop { x: 0.0, y: 0.0, width: 0.5, height: 0.5 }),
            ..Default::default()
        };
        let out = apply_edits(img, &e);
        assert_eq!(out.dimensions(), (100, 200));
    }

    #[test]
    fn edits_validate() {
        assert!(Edits { rotate: 45, ..Default::default() }.validate().is_err());
        assert!(Edits { brightness: 20.0, filter: Some("bw".into()), ..Default::default() }.validate().is_ok());
        assert!(Edits::default().is_identity());
    }

    #[test]
    fn finds_motion_video() {
        let mut bytes = vec![0xFFu8, 0xD8];
        bytes.extend(std::iter::repeat_n(0u8, 2000));
        let start = bytes.len();
        bytes.extend([0, 0, 0, 24]);
        bytes.extend(b"ftypmp42");
        bytes.extend([0u8; 100]);
        assert_eq!(find_embedded_video(&bytes, None), Some(start));
        assert_eq!(find_embedded_video(&bytes, Some(bytes.len() - start)), Some(start));
    }

    #[test]
    fn exposure_formatting() {
        assert_eq!(format_exposure(0.004), "1/250");
        assert_eq!(format_exposure(2.0), "2");
    }
}
