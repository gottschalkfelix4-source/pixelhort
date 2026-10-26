//! Database row types and their API representations.

use base64::Engine;
use chrono::{DateTime, Utc};
use pixelhort_common::{AssetDto, ExifDto, PersonRef, TagRef, UserDto, UserSummary};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::auth::UserRow;

/// Distinguishes a missing field (`None`) from an explicit `null` (`Some(None)`).
/// Use with `#[serde(default, with = "crate::model::double_option")]`.
pub mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct AssetRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub library_id: Option<Uuid>,
    pub device_asset_id: Option<String>,
    pub device_id: Option<String>,
    pub asset_type: String,
    pub original_path: String,
    pub original_file_name: String,
    pub original_mime_type: Option<String>,
    pub sidecar_path: Option<String>,
    pub checksum: String,
    pub file_size: i64,
    pub file_created_at: DateTime<Utc>,
    pub file_modified_at: DateTime<Utc>,
    pub local_date_time: DateTime<Utc>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_ms: Option<i64>,
    pub is_favorite: bool,
    pub visibility: String,
    pub deleted_at: Option<DateTime<Utc>>,
    pub is_offline: bool,
    pub live_photo_video_id: Option<Uuid>,
    #[allow(dead_code)]
    pub content_identifier: Option<String>,
    pub is_motion_photo: bool,
    pub projection_type: Option<String>,
    pub is_panorama: bool,
    pub is_raw: bool,
    pub thumbhash: Option<Vec<u8>>,
    pub thumbnail_path: Option<String>,
    pub preview_path: Option<String>,
    pub encoded_video_path: Option<String>,
    pub rating: Option<i16>,
    pub description: String,
    pub edits: Option<serde_json::Value>,
    pub duplicate_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Column list for `SELECT` statements on `assets a`.
pub const ASSET_COLUMNS: &str = "a.id, a.owner_id, a.library_id, a.device_asset_id, a.device_id, a.asset_type, \
 a.original_path, a.original_file_name, a.original_mime_type, a.sidecar_path, a.checksum, a.file_size, \
 a.file_created_at, a.file_modified_at, a.local_date_time, a.width, a.height, a.duration_ms, a.is_favorite, \
 a.visibility, a.deleted_at, a.is_offline, a.live_photo_video_id, a.content_identifier, a.is_motion_photo, \
 a.projection_type, a.is_panorama, a.is_raw, a.thumbhash, a.thumbnail_path, a.preview_path, \
 a.encoded_video_path, a.rating, a.description, a.edits, a.duplicate_id, a.created_at, a.updated_at";

#[derive(Debug, Clone, FromRow)]
pub struct ExifRow {
    pub asset_id: Uuid,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens_model: Option<String>,
    pub f_number: Option<f64>,
    pub exposure_time: Option<String>,
    pub iso: Option<i32>,
    pub focal_length: Option<f64>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub orientation: Option<i32>,
    pub date_time_original: Option<DateTime<Utc>>,
    pub time_zone: Option<String>,
    pub projection_type: Option<String>,
    pub description: Option<String>,
}

impl From<ExifRow> for ExifDto {
    fn from(e: ExifRow) -> Self {
        ExifDto {
            make: e.make,
            model: e.model,
            lens_model: e.lens_model,
            f_number: e.f_number,
            exposure_time: e.exposure_time,
            iso: e.iso,
            focal_length: e.focal_length,
            latitude: e.latitude,
            longitude: e.longitude,
            altitude: e.altitude,
            city: e.city,
            state: e.state,
            country: e.country,
            orientation: e.orientation,
            date_time_original: e.date_time_original,
            time_zone: e.time_zone,
            projection_type: e.projection_type,
            description: e.description,
        }
    }
}

impl AssetRow {
    pub fn to_dto(&self) -> AssetDto {
        AssetDto {
            id: self.id,
            owner_id: self.owner_id,
            library_id: self.library_id,
            device_asset_id: self.device_asset_id.clone(),
            device_id: self.device_id.clone(),
            asset_type: self.asset_type.clone(),
            original_file_name: self.original_file_name.clone(),
            original_mime_type: self.original_mime_type.clone(),
            checksum: self.checksum.clone(),
            file_size: self.file_size,
            file_created_at: self.file_created_at,
            file_modified_at: self.file_modified_at,
            local_date_time: self.local_date_time,
            created_at: self.created_at,
            updated_at: self.updated_at,
            width: self.width,
            height: self.height,
            duration: self.duration_ms.map(|ms| ms as f64 / 1000.0),
            is_favorite: self.is_favorite,
            visibility: self.visibility.clone(),
            is_trashed: self.deleted_at.is_some(),
            is_offline: self.is_offline,
            live_photo_video_id: self.live_photo_video_id,
            is_motion_photo: self.is_motion_photo,
            projection_type: self.projection_type.clone(),
            is_panorama: self.is_panorama,
            is_raw: self.is_raw,
            thumbhash: self.thumbhash.as_ref().map(|t| base64::engine::general_purpose::STANDARD.encode(t)),
            rating: self.rating,
            description: self.description.clone(),
            has_edits: self.edits.as_ref().map(|e| !e.is_null()).unwrap_or(false),
            duplicate_id: self.duplicate_id,
            exif: None,
            tags: vec![],
            people: vec![],
        }
    }

    pub fn is_video(&self) -> bool {
        self.asset_type == "VIDEO"
    }
}

pub fn assets_to_dtos(rows: &[AssetRow]) -> Vec<AssetDto> {
    rows.iter().map(AssetRow::to_dto).collect()
}

pub async fn get_asset(db: &PgPool, id: Uuid) -> sqlx::Result<AssetRow> {
    sqlx::query_as(&format!("SELECT {ASSET_COLUMNS} FROM assets a WHERE a.id = $1"))
        .bind(id)
        .fetch_one(db)
        .await
}

/// Full asset DTO with EXIF, tags and people.
pub async fn full_asset_dto(db: &PgPool, row: &AssetRow, include_metadata: bool) -> sqlx::Result<AssetDto> {
    let mut dto = row.to_dto();
    if include_metadata {
        let exif: Option<ExifRow> = sqlx::query_as("SELECT * FROM exif WHERE asset_id = $1")
            .bind(row.id)
            .fetch_optional(db)
            .await?;
        dto.exif = exif.map(Into::into);
        dto.tags = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
            "SELECT t.id, t.value, t.color FROM tags t JOIN tag_assets ta ON ta.tag_id = t.id WHERE ta.asset_id = $1 ORDER BY t.value",
        )
        .bind(row.id)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|(id, value, color)| TagRef { id, value, color })
        .collect();
        dto.people = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT DISTINCT p.id, p.name FROM people p JOIN faces f ON f.person_id = p.id WHERE f.asset_id = $1 AND NOT p.is_hidden",
        )
        .bind(row.id)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(|(id, name)| PersonRef { id, name })
        .collect();
    }
    Ok(dto)
}

pub async fn user_dto(db: &PgPool, u: &UserRow) -> sqlx::Result<UserDto> {
    let usage: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(SUM(file_size), 0)::BIGINT FROM assets WHERE owner_id = $1 AND library_id IS NULL",
    )
    .bind(u.id)
    .fetch_one(db)
    .await?;
    Ok(UserDto {
        id: u.id,
        email: u.email.clone(),
        name: u.name.clone(),
        is_admin: u.is_admin,
        quota_bytes: u.quota_bytes,
        usage_bytes: usage.unwrap_or(0),
        has_pin: u.pin_hash.is_some(),
        oauth_id: u.oauth_id.clone(),
        created_at: u.created_at,
        updated_at: u.updated_at,
    })
}

pub async fn user_summary(db: &PgPool, id: Uuid) -> sqlx::Result<UserSummary> {
    let (id, name, email): (Uuid, String, String) =
        sqlx::query_as("SELECT id, name, email FROM users WHERE id = $1").bind(id).fetch_one(db).await?;
    Ok(UserSummary { id, name, email })
}
