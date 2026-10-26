//! Generates preview (1440px JPEG), thumbnail (250px JPEG) and thumbhash.

use std::path::{Path, PathBuf};

use image::{DynamicImage, GenericImageView};
use uuid::Uuid;

use crate::media::{self, Edits};
use crate::model::{AssetRow, get_asset};
use crate::state::SharedState;

pub const PREVIEW_SIZE: u32 = 1440;
pub const THUMB_SIZE: u32 = 250;

pub fn edits_of(asset: &AssetRow) -> Option<Edits> {
    asset.edits.clone().and_then(|v| serde_json::from_value(v).ok()).filter(|e: &Edits| !e.is_identity())
}

/// Loads the original upright, with edits applied.
pub async fn render(state: &SharedState, asset: &AssetRow, apply_edits: bool) -> anyhow::Result<DynamicImage> {
    let orientation: Option<i32> = sqlx::query_scalar("SELECT orientation FROM exif WHERE asset_id = $1")
        .bind(asset.id)
        .fetch_optional(&state.db)
        .await?
        .flatten();
    let duration = asset.duration_ms.map(|d| d as f64 / 1000.0);
    let img = media::load_upright(Path::new(&asset.original_path), asset.is_video(), orientation, duration).await?;
    Ok(match (apply_edits, edits_of(asset)) {
        (true, Some(e)) => tokio::task::spawn_blocking(move || media::apply_edits(img, &e)).await?,
        _ => img,
    })
}

async fn write_atomic(path: &PathBuf, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await
}

pub async fn run(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let asset = get_asset(&state.db, asset_id).await?;
    let img = render(state, &asset, true).await?;
    let (full_w, full_h) = img.dimensions();
    let (preview, thumb, hash) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let preview = media::fit_longest(&img, PREVIEW_SIZE);
        let thumb = media::fit_shortest(&preview, THUMB_SIZE);
        Ok((media::encode_jpeg(&preview, 82)?, media::encode_jpeg(&thumb, 78)?, media::thumbhash(&thumb)))
    })
    .await??;

    let preview_path = state.storage.thumb_path(asset.owner_id, asset.id, "preview");
    let thumb_path = state.storage.thumb_path(asset.owner_id, asset.id, "thumbnail");
    write_atomic(&preview_path, &preview).await?;
    write_atomic(&thumb_path, &thumb).await?;

    // Without metadata (or for edited assets) the rendered size is the best we know.
    let has_edits = edits_of(&asset).is_some();
    sqlx::query(
        "UPDATE assets SET preview_path = $2, thumbnail_path = $3, thumbhash = $4, thumbnails_at = now(),
            width = CASE WHEN $7 OR width IS NULL THEN $5 ELSE width END,
            height = CASE WHEN $7 OR height IS NULL THEN $6 ELSE height END,
            updated_at = now()
         WHERE id = $1",
    )
    .bind(asset_id)
    .bind(preview_path.to_string_lossy().to_string())
    .bind(thumb_path.to_string_lossy().to_string())
    .bind(hash)
    .bind(full_w as i32)
    .bind(full_h as i32)
    .bind(has_edits)
    .execute(&state.db)
    .await?;

    if asset.visibility != "hidden" && asset.smart_search_at_missing(state).await? {
        crate::jobs::enqueue_ml(state, asset_id, &asset.asset_type).await?;
    }
    Ok(())
}

impl AssetRow {
    async fn smart_search_at_missing(&self, state: &SharedState) -> sqlx::Result<bool> {
        let missing: bool = sqlx::query_scalar("SELECT smart_search_at IS NULL OR faces_at IS NULL FROM assets WHERE id = $1")
            .bind(self.id)
            .fetch_one(&state.db)
            .await?;
        Ok(missing)
    }
}
