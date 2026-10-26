//! Permission checks shared by the route handlers.

use uuid::Uuid;

use crate::auth::{Auth, Principal};
use crate::error::{AppError, AppResult};
use crate::model::AssetRow;
use crate::state::SharedState;

/// Asset ids that grant access to `asset` (itself plus the still image of a live photo).
async fn related_ids(state: &SharedState, asset: &AssetRow) -> AppResult<Vec<Uuid>> {
    let mut ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM assets WHERE live_photo_video_id = $1")
        .bind(asset.id)
        .fetch_all(&state.db)
        .await?;
    ids.push(asset.id);
    Ok(ids)
}

pub async fn user_can_read(state: &SharedState, user_id: Uuid, unlocked: bool, asset: &AssetRow) -> AppResult<bool> {
    if asset.owner_id == user_id {
        return Ok(asset.visibility != "locked" || unlocked);
    }
    if asset.visibility == "locked" {
        return Ok(false);
    }
    let ids = related_ids(state, asset).await?;
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM partners p WHERE p.shared_by_id = $1 AND p.shared_with_id = $2 AND $3 IN ('timeline', 'hidden')
         ) OR EXISTS (
            SELECT 1 FROM album_assets aa JOIN albums al ON al.id = aa.album_id
            LEFT JOIN album_users au ON au.album_id = al.id AND au.user_id = $2
            WHERE aa.asset_id = ANY($4) AND (al.owner_id = $2 OR au.user_id IS NOT NULL)
         )",
    )
    .bind(asset.owner_id)
    .bind(user_id)
    .bind(&asset.visibility)
    .bind(&ids)
    .fetch_one(&state.db)
    .await?;
    Ok(ok)
}

pub async fn share_can_read(state: &SharedState, link_id: Uuid, album_id: Option<Uuid>, asset: &AssetRow) -> AppResult<bool> {
    if asset.visibility == "locked" {
        return Ok(false);
    }
    let ids = related_ids(state, asset).await?;
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shared_link_assets WHERE shared_link_id = $1 AND asset_id = ANY($3))
             OR EXISTS (SELECT 1 FROM album_assets WHERE album_id = $2 AND asset_id = ANY($3))",
    )
    .bind(link_id)
    .bind(album_id)
    .bind(&ids)
    .fetch_one(&state.db)
    .await?;
    Ok(ok)
}

/// Read access for any principal.
pub async fn require_read(state: &SharedState, principal: &Principal, asset: &AssetRow) -> AppResult<()> {
    let ok = match principal {
        Principal::User { user, unlocked, .. } => user_can_read(state, user.id, *unlocked, asset).await?,
        Principal::Share { link } => share_can_read(state, link.id, link.album_id, asset).await?,
        Principal::Cast { asset_id } => {
            *asset_id == asset.id || related_ids(state, asset).await?.contains(asset_id)
        }
    };
    if ok {
        Ok(())
    } else if asset.visibility == "locked" {
        Err(AppError::Forbidden("locked".into()))
    } else {
        Err(AppError::not_found())
    }
}

/// Write access: only the owner (and the locked folder must be unlocked for locked assets).
pub fn require_owner(auth: &Auth, asset: &AssetRow) -> AppResult<()> {
    if asset.owner_id != auth.id() {
        return Err(AppError::forbidden());
    }
    if asset.visibility == "locked" {
        auth.require_unlocked()?;
    }
    Ok(())
}

/// Returns the caller's role in an album: "owner", "editor", "viewer".
pub async fn album_role(state: &SharedState, user_id: Uuid, album_id: Uuid) -> AppResult<Option<String>> {
    let role: Option<Option<String>> = sqlx::query_scalar(
        "SELECT CASE WHEN al.owner_id = $2 THEN 'owner' ELSE au.role END
         FROM albums al LEFT JOIN album_users au ON au.album_id = al.id AND au.user_id = $2
         WHERE al.id = $1",
    )
    .bind(album_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(role.flatten())
}

pub async fn require_album_role(state: &SharedState, user_id: Uuid, album_id: Uuid, write: bool) -> AppResult<String> {
    match album_role(state, user_id, album_id).await? {
        None => Err(AppError::not_found()),
        Some(r) if write && r == "viewer" => Err(AppError::forbidden()),
        Some(r) => Ok(r),
    }
}

/// Ids of users whose libraries `user_id` may see as partner.
pub async fn partner_ids(state: &SharedState, user_id: Uuid, timeline_only: bool) -> AppResult<Vec<Uuid>> {
    let sql = if timeline_only {
        "SELECT shared_by_id FROM partners WHERE shared_with_id = $1 AND in_timeline"
    } else {
        "SELECT shared_by_id FROM partners WHERE shared_with_id = $1"
    };
    Ok(sqlx::query_scalar(sql).bind(user_id).fetch_all(&state.db).await?)
}
