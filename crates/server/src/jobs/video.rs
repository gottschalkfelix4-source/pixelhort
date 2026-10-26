//! Transcodes videos that browsers cannot play natively to H.264/AAC MP4.

use std::path::Path;

use uuid::Uuid;

use crate::media;
use crate::model::get_asset;
use crate::state::SharedState;

/// True if the original can be streamed as-is to browsers and phones.
pub fn is_web_friendly(p: &media::Probe) -> bool {
    let container_ok = p.format.as_deref().map(|f| f.contains("mp4") || f.contains("mov")).unwrap_or(false);
    let video_ok = p.video_codec.as_deref() == Some("h264");
    let audio_ok = matches!(p.audio_codec.as_deref(), None | Some("aac") | Some("mp3"));
    container_ok && video_ok && audio_ok
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
        let scale = format!(
            "scale=w='if(gt(iw,ih),-2,min({r},iw))':h='if(gt(iw,ih),min({r},ih),-2)'",
            r = cfg.target_resolution
        );
        let crf = cfg.crf.to_string();
        let status = tokio::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(src)
            .args(["-map", "0:v:0", "-map", "0:a:0?", "-c:v", "libx264", "-preset", &cfg.preset, "-crf", &crf])
            .args(["-pix_fmt", "yuv420p", "-vf", &scale, "-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart"])
            .arg(&tmp)
            .stdin(std::process::Stdio::null())
            .output()
            .await?;
        if !status.status.success() {
            let _ = tokio::fs::remove_file(&tmp).await;
            anyhow::bail!("ffmpeg: {}", String::from_utf8_lossy(&status.stderr).trim());
        }
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
}
