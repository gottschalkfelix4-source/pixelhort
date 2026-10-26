//! API data transfer objects shared by the server and the CLI.
//! The wire format is documented in `docs/API.md`.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const API_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_admin: bool,
    pub quota_bytes: Option<i64>,
    pub usage_bytes: i64,
    pub has_pin: bool,
    pub oauth_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSummary {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub device_type: Option<String>,
    #[serde(default)]
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub access_token: String,
    pub user: UserDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifDto {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRef {
    pub id: Uuid,
    pub value: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub library_id: Option<Uuid>,
    pub device_asset_id: Option<String>,
    pub device_id: Option<String>,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub original_file_name: String,
    pub original_mime_type: Option<String>,
    pub checksum: String,
    pub file_size: i64,
    pub file_created_at: DateTime<Utc>,
    pub file_modified_at: DateTime<Utc>,
    pub local_date_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration: Option<f64>,
    pub is_favorite: bool,
    pub visibility: String,
    pub is_trashed: bool,
    pub is_offline: bool,
    pub live_photo_video_id: Option<Uuid>,
    pub is_motion_photo: bool,
    pub projection_type: Option<String>,
    pub is_panorama: bool,
    pub is_raw: bool,
    pub thumbhash: Option<String>,
    pub rating: Option<i16>,
    pub description: String,
    pub has_edits: bool,
    pub duplicate_id: Option<Uuid>,
    pub exif: Option<ExifDto>,
    pub tags: Vec<TagRef>,
    pub people: Vec<PersonRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    pub id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUploadCheckItem {
    pub id: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUploadCheckRequest {
    pub assets: Vec<BulkUploadCheckItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUploadCheckResult {
    pub id: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkUploadCheckResponse {
    pub results: Vec<BulkUploadCheckResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumUserDto {
    pub user: UserSummary,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner: UserSummary,
    pub album_name: String,
    pub description: String,
    pub album_thumbnail_asset_id: Option<Uuid>,
    pub asset_count: i64,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub shared: bool,
    pub order: String,
    pub album_users: Vec<AlbumUserDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<AssetDto>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAlbumRequest {
    pub album_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub asset_ids: Vec<Uuid>,
    #[serde(default)]
    pub album_users: Vec<AlbumUserRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumUserRequest {
    pub user_id: Uuid,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "editor".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdsRequest {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkIdResult {
    pub id: Uuid,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerFeatures {
    pub smart_search: bool,
    pub faces: bool,
    pub ocr: bool,
    pub objects: bool,
    pub oauth: bool,
    pub reverse_geocoding: bool,
    pub cast: bool,
    #[serde(default)]
    pub map: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub version: String,
    pub features: ServerFeatures,
    pub public_url: Option<String>,
    #[serde(default)]
    pub map_tile_url: String,
    #[serde(default)]
    pub map_attribution: String,
    /// Days until trashed assets are deleted permanently (0 = never).
    #[serde(default)]
    pub trash_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeBucket {
    pub time_bucket: NaiveDate,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub import_paths: Vec<String>,
    pub exclusion_patterns: Vec<String>,
    pub watch: bool,
    pub asset_count: i64,
    pub refreshed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Lower-case file extensions the server accepts as media.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "png", "gif", "webp", "avif", "heic", "heif", "hif", "tif", "tiff", "bmp",
    "jxl", "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2",
    "pef", "srw", "3fr", "iiq", "erf", "kdc", "dcr", "mrw", "x3f", "raw", "rwl", "insp",
];
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "qt", "3gp", "3gpp", "avi", "mkv", "webm", "mts", "m2ts", "mpg", "mpeg",
    "wmv", "flv", "insv", "mxf", "vob",
];
pub const RAW_EXTENSIONS: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef",
    "srw", "3fr", "iiq", "erf", "kdc", "dcr", "mrw", "x3f", "raw", "rwl",
];

/// Returns "IMAGE" / "VIDEO" for supported file names.
pub fn media_type_for(file_name: &str) -> Option<&'static str> {
    let ext = file_name.rsplit_once('.')?.1.to_ascii_lowercase();
    if IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        Some("IMAGE")
    } else if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
        Some("VIDEO")
    } else {
        None
    }
}

pub fn is_raw(file_name: &str) -> bool {
    file_name
        .rsplit_once('.')
        .map(|(_, e)| RAW_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_media_types() {
        assert_eq!(media_type_for("IMG_0001.HEIC"), Some("IMAGE"));
        assert_eq!(media_type_for("clip.MOV"), Some("VIDEO"));
        assert_eq!(media_type_for("notes.txt"), None);
        assert_eq!(media_type_for("noext"), None);
        assert!(is_raw("DSC0001.ARW"));
        assert!(!is_raw("a.jpg"));
    }
}
