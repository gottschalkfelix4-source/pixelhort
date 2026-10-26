//! Transcodes videos that browsers cannot play natively to H.264/AAC MP4, optionally on
//! the GPU (VAAPI or Intel Quick Sync) with a fallback to software encoding.

use std::ffi::OsString;
use std::path::Path;

use anyhow::Context;
use uuid::Uuid;

use crate::media;
use crate::model::get_asset;
use crate::state::SharedState;
use crate::sysconfig::VideoConfig;

/// True if the original can be streamed as-is to browsers and phones.
pub fn is_web_friendly(p: &media::Probe) -> bool {
    let container_ok = p.format.as_deref().map(|f| f.contains("mp4") || f.contains("mov")).unwrap_or(false);
    let video_ok = p.video_codec.as_deref() == Some("h264");
    let audio_ok = matches!(p.audio_codec.as_deref(), None | Some("aac") | Some("mp3"));
    container_ok && video_ok && audio_ok
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Encoder {
    Software,
    Vaapi { low_power: bool },
    /// VAAPI decoding and scaling, Quick Sync encoding.
    Qsv,
}

impl Encoder {
    fn label(self) -> &'static str {
        match self {
            Encoder::Software => "software (libx264)",
            Encoder::Vaapi { low_power: true } => "VAAPI (low power)",
            Encoder::Vaapi { low_power: false } => "VAAPI",
            Encoder::Qsv => "Quick Sync",
        }
    }
}

/// The VAAPI render node to use, if hardware acceleration is enabled and the device exists.
pub fn gpu_device(cfg: &VideoConfig) -> Option<String> {
    (cfg.accel != "none" && Path::new(&cfg.accel_device).exists()).then(|| cfg.accel_device.clone())
}

/// Encoders to try, in order.
fn encoders(cfg: &VideoConfig) -> Vec<Encoder> {
    match cfg.accel.as_str() {
        // Arc and recent iGPUs only encode in low-power (VDEnc) mode, older ones only without it.
        "vaapi" => vec![Encoder::Vaapi { low_power: true }, Encoder::Vaapi { low_power: false }, Encoder::Software],
        "qsv" => vec![Encoder::Qsv, Encoder::Software],
        _ => vec![Encoder::Software],
    }
}

/// Upright output size: the shorter side capped at `target` (never upscaled), even dimensions.
fn target_size(p: &media::Probe, target: u32) -> Option<(u32, u32)> {
    let (w, h) = (u32::try_from(p.width?).ok()?, u32::try_from(p.height?).ok()?);
    if w == 0 || h == 0 {
        return None;
    }
    let (w, h) = if p.rotation % 180 == 90 { (h, w) } else { (w, h) };
    let short = target.min(w.min(h)) as f64;
    let even = |v: f64| ((v / 2.0).round() as u32 * 2).max(2);
    Some(if w > h {
        (even(w as f64 * short / h as f64), even(short))
    } else {
        (even(short), even(h as f64 * short / w as f64))
    })
}

fn ffmpeg_args(enc: Encoder, src: &Path, dst: &Path, p: &media::Probe, cfg: &VideoConfig) -> Option<Vec<OsString>> {
    let push = |a: &mut Vec<OsString>, args: &[&str]| a.extend(args.iter().map(OsString::from));
    let mut a = Vec::new();
    push(&mut a, &["-hide_banner", "-loglevel", "error", "-y"]);
    let crf = cfg.crf.to_string();
    if enc == Encoder::Software {
        let scale = format!(
            "scale=w='if(gt(iw,ih),-2,min({r},iw))':h='if(gt(iw,ih),min({r},ih),-2)'",
            r = cfg.target_resolution
        );
        push(&mut a, &["-i"]);
        a.push(src.into());
        push(&mut a, &["-map", "0:v:0", "-map", "0:a:0?", "-c:v", "libx264", "-preset", &cfg.preset, "-crf", &crf]);
        push(&mut a, &["-pix_fmt", "yuv420p", "-vf", &scale]);
    } else {
        let (w, h) = target_size(p, cfg.target_resolution)?;
        let dev = format!("vaapi=va:{}", cfg.accel_device);
        push(&mut a, &["-init_hw_device", &dev]);
        if enc == Encoder::Qsv {
            push(&mut a, &["-init_hw_device", "qsv=qs@va", "-filter_hw_device", "qs"]);
        } else {
            push(&mut a, &["-filter_hw_device", "va"]);
        }
        // The GPU filters cannot follow ffmpeg's automatic (software) rotation, so rotate
        // explicitly and drop the rotation tag from the output.
        push(&mut a, &["-hwaccel", "vaapi", "-hwaccel_device", "va", "-hwaccel_output_format", "vaapi"]);
        push(&mut a, &["-noautorotate", "-display_rotation", "0", "-i"]);
        a.push(src.into());
        let mut vf = match p.rotation {
            90 => "transpose_vaapi=dir=clock,".to_string(),
            180 => "transpose_vaapi=dir=reversal,".to_string(),
            270 => "transpose_vaapi=dir=cclock,".to_string(),
            _ => String::new(),
        };
        vf += &format!("scale_vaapi=w={w}:h={h}:format=nv12");
        push(&mut a, &["-map", "0:v:0", "-map", "0:a:0?"]);
        match enc {
            Encoder::Vaapi { low_power } => {
                push(&mut a, &["-vf", &vf, "-c:v", "h264_vaapi", "-rc_mode", "CQP", "-qp", &crf]);
                if low_power {
                    push(&mut a, &["-low_power", "1"]);
                }
            }
            _ => {
                vf += ",hwmap=derive_device=qsv,format=qsv";
                // Quick Sync knows the x264 preset names from veryfast on.
                let preset: &str = if matches!(cfg.preset.as_str(), "ultrafast" | "superfast") { "veryfast" } else { &cfg.preset };
                push(&mut a, &["-vf", &vf, "-c:v", "h264_qsv", "-preset", preset, "-q:v", &crf]);
            }
        }
    }
    push(&mut a, &["-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart"]);
    a.push(dst.into());
    Some(a)
}

async fn ffmpeg(args: &[OsString]) -> anyhow::Result<()> {
    let out = tokio::process::Command::new("ffmpeg")
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .await?;
    if !out.status.success() {
        anyhow::bail!("ffmpeg: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

async fn transcode(src: &Path, dst: &Path, p: &media::Probe, cfg: &VideoConfig) -> anyhow::Result<()> {
    let mut encoders = encoders(cfg);
    if cfg.accel != "none" && gpu_device(cfg).is_none() {
        tracing::warn!(device = %cfg.accel_device, "GPU device not found (is /dev/dri passed to the container?), transcoding in software");
        encoders.retain(|e| *e == Encoder::Software);
    }
    let mut hw_err = None;
    for enc in encoders {
        let Some(args) = ffmpeg_args(enc, src, dst, p, cfg) else { continue };
        if enc == Encoder::Software {
            if let Some(e) = hw_err.take() {
                tracing::warn!(file = %src.display(), "hardware transcoding failed, falling back to software: {e:#}");
            }
        }
        match ffmpeg(&args).await {
            Ok(()) => {
                tracing::info!(file = %src.display(), encoder = enc.label(), "video transcoded");
                return Ok(());
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(dst).await;
                if enc == Encoder::Software {
                    return Err(e);
                }
                tracing::debug!(?enc, "transcoding failed: {e:#}");
                hw_err = Some(e);
            }
        }
    }
    Err(hw_err.unwrap_or_else(|| anyhow::anyhow!("no usable encoder")))
}

/// Transcodes a generated test clip with the configured GPU pipeline (no software fallback)
/// and returns the encoder that worked.
pub async fn test_accel(cfg: &VideoConfig) -> anyhow::Result<&'static str> {
    if cfg.accel == "none" {
        anyhow::bail!("no hardware acceleration selected");
    }
    if gpu_device(cfg).is_none() {
        anyhow::bail!("{} not found (is /dev/dri passed to the container?)", cfg.accel_device);
    }
    let dir = std::env::temp_dir().join(format!("ph-accel-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir).await?;
    let (src, dst) = (dir.join("in.mp4"), dir.join("out.mp4"));
    let res = async {
        let mut clip: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=30:duration=1"]
            .into_iter()
            .chain(["-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p"])
            .map(OsString::from)
            .collect();
        clip.push(src.clone().into());
        ffmpeg(&clip).await.context("creating the test clip")?;
        let probe = media::ffprobe(&src).await?;
        let mut errors = Vec::new();
        for enc in encoders(cfg).into_iter().filter(|e| *e != Encoder::Software) {
            let Some(args) = ffmpeg_args(enc, &src, &dst, &probe, cfg) else { continue };
            match ffmpeg(&args).await {
                Ok(()) => return Ok(enc.label()),
                Err(e) => errors.push(format!("{}: {e:#}", enc.label())),
            }
        }
        anyhow::bail!("{}", errors.join("\n"))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    res
}

pub async fn run(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let asset = get_asset(&state.db, asset_id).await?;
    let src = Path::new(&asset.original_path);
    let probe = media::ffprobe(src).await?;
    let out = state.storage.encoded_video_path(asset.owner_id, asset.id);
    let encoded = if is_web_friendly(&probe) {
        let _ = tokio::fs::remove_file(&out).await;
        None
    } else {
        let cfg = state.sys().video;
        tokio::fs::create_dir_all(out.parent().expect("parent")).await?;
        let tmp = out.with_extension("part.mp4");
        transcode(src, &tmp, &probe, &cfg).await?;
        tokio::fs::rename(&tmp, &out).await?;
        Some(out.to_string_lossy().to_string())
    };
    sqlx::query(
        "UPDATE assets SET encoded_video_path = $2, video_at = now(),
            duration_ms = COALESCE(duration_ms, $3), updated_at = now() WHERE id = $1",
    )
    .bind(asset_id)
    .bind(encoded)
    .bind(probe.duration.map(|d| (d * 1000.0) as i64))
    .execute(&state.db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_friendly_detection() {
        let mut p = media::Probe {
            format: Some("mov,mp4,m4a,3gp,3g2,mj2".into()),
            video_codec: Some("h264".into()),
            audio_codec: Some("aac".into()),
            ..Default::default()
        };
        assert!(is_web_friendly(&p));
        p.video_codec = Some("hevc".into());
        assert!(!is_web_friendly(&p));
    }

    fn probe(w: i32, h: i32, rotation: i32) -> media::Probe {
        media::Probe { width: Some(w), height: Some(h), rotation, ..Default::default() }
    }

    #[test]
    fn target_size_caps_shorter_side() {
        assert_eq!(target_size(&probe(3840, 2160, 0), 720), Some((1280, 720)));
        // Portrait iPhone clip: stored landscape, rotated 90° clockwise.
        assert_eq!(target_size(&probe(3840, 2160, 90), 720), Some((720, 1280)));
        assert_eq!(target_size(&probe(640, 480, 0), 720), Some((640, 480)));
        assert_eq!(target_size(&probe(1920, 1080, 0), 1080), Some((1920, 1080)));
        assert_eq!(target_size(&media::Probe::default(), 720), None);
    }

    fn joined(enc: Encoder, p: &media::Probe, cfg: &VideoConfig) -> String {
        let args = ffmpeg_args(enc, Path::new("/in.mov"), Path::new("/out.mp4"), p, cfg).expect("args");
        args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn hardware_pipelines() {
        let cfg = VideoConfig { accel: "vaapi".into(), ..Default::default() };
        assert_eq!(encoders(&cfg)[0], Encoder::Vaapi { low_power: true });
        let p = probe(3840, 2160, 90);
        let vaapi = joined(Encoder::Vaapi { low_power: true }, &p, &cfg);
        assert!(vaapi.contains("-init_hw_device vaapi=va:/dev/dri/renderD128"));
        assert!(vaapi.contains("-noautorotate -display_rotation 0 -i /in.mov"));
        assert!(vaapi.contains("-vf transpose_vaapi=dir=clock,scale_vaapi=w=720:h=1280:format=nv12 -c:v h264_vaapi"));
        assert!(vaapi.contains("-low_power 1"));
        assert!(!joined(Encoder::Vaapi { low_power: false }, &p, &cfg).contains("-low_power"));

        let cfg = VideoConfig { accel: "qsv".into(), preset: "ultrafast".into(), ..Default::default() };
        let qsv = joined(Encoder::Qsv, &probe(1920, 1080, 0), &cfg);
        assert!(qsv.contains("-vf scale_vaapi=w=1280:h=720:format=nv12,hwmap=derive_device=qsv,format=qsv"));
        assert!(qsv.contains("-c:v h264_qsv -preset veryfast"));
        assert_eq!(encoders(&cfg), vec![Encoder::Qsv, Encoder::Software]);

        // Unknown size: no GPU pipeline, software still works.
        assert!(ffmpeg_args(Encoder::Qsv, Path::new("/a"), Path::new("/b"), &media::Probe::default(), &cfg).is_none());
        assert!(joined(Encoder::Software, &media::Probe::default(), &cfg).contains("-c:v libx264"));
    }
}
