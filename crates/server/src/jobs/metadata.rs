//! Metadata extraction (exiftool, ffprobe fallback), reverse geocoding,
//! live/motion photo linking and keyword import.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde_json::{Map, Value};
use sha1::{Digest, Sha1};
use uuid::Uuid;

use crate::media;
use crate::model::{AssetRow, get_asset};
use crate::state::SharedState;

type Tags = Map<String, Value>;

fn s(tags: &Tags, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| match tags.get(*k)? {
        Value::String(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

fn f(tags: &Tags, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| match tags.get(*k)? {
        Value::Number(n) => n.as_f64(),
        Value::String(v) => v.trim().parse().ok(),
        _ => None,
    })
}

/// Parses exiftool dates like `2024:05:01 12:00:00.123+02:00`.
/// Returns (wall clock time, offset if known).
pub fn parse_exif_date(value: &str) -> Option<(NaiveDateTime, Option<FixedOffset>)> {
    let v = value.trim();
    if v.len() < 19 || v.starts_with("0000") {
        return None;
    }
    let naive = NaiveDateTime::parse_from_str(&v[..19], "%Y:%m:%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(&v[..19], "%Y-%m-%dT%H:%M:%S"))
        .ok()?;
    let rest = v[19..].trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    Some((naive, parse_offset(rest)))
}

pub fn parse_offset(v: &str) -> Option<FixedOffset> {
    let v = v.trim();
    if v == "Z" {
        return FixedOffset::east_opt(0);
    }
    let sign = match v.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let (h, m) = v[1..].split_once(':').unwrap_or((&v[1..v.len().min(3)], "0"));
    let secs = h.parse::<i32>().ok()? * 3600 + m.parse::<i32>().ok()? * 60;
    FixedOffset::east_opt(sign * secs)
}

#[derive(Debug, Default)]
pub struct Extracted {
    pub local: Option<NaiveDateTime>,
    pub utc: Option<DateTime<Utc>>,
    pub time_zone: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration_ms: Option<i64>,
    pub orientation: Option<i32>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub f_number: Option<f64>,
    pub exposure: Option<String>,
    pub iso: Option<i32>,
    pub focal_length: Option<f64>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt: Option<f64>,
    pub projection: Option<String>,
    pub is_panorama: bool,
    pub description: Option<String>,
    pub rating: Option<i16>,
    pub content_id: Option<String>,
    pub motion: bool,
    pub micro_video_offset: Option<usize>,
    pub keywords: Vec<String>,
    pub mime: Option<String>,
}

pub fn extract(tags: &Tags, is_video: bool) -> Extracted {
    let mut e = Extracted::default();
    let date_keys: &[&str] = if is_video {
        &["CreationDate", "DateTimeOriginal", "SubSecDateTimeOriginal", "MediaCreateDate", "CreateDate"]
    } else {
        &["SubSecDateTimeOriginal", "DateTimeOriginal", "SubSecCreateDate", "CreateDate", "CreationDate", "DateTimeCreated", "DateCreated", "ModifyDate"]
    };
    for key in date_keys {
        if let Some((naive, offset)) = s(tags, &[key]).as_deref().and_then(parse_exif_date) {
            let offset = offset.or_else(|| {
                s(tags, &["OffsetTimeOriginal", "OffsetTime", "OffsetTimeDigitized"]).as_deref().and_then(parse_offset)
            });
            // QuickTime CreateDate/MediaCreateDate are UTC by specification.
            let quicktime_utc = is_video && (*key == "CreateDate" || *key == "MediaCreateDate") && offset.is_none();
            match offset {
                Some(off) => {
                    e.utc = off.from_local_datetime(&naive).single().map(|d| d.with_timezone(&Utc));
                    e.local = Some(naive);
                    e.time_zone = Some(off.to_string());
                }
                None if quicktime_utc => {
                    e.utc = Some(Utc.from_utc_datetime(&naive));
                    e.local = Some(naive);
                }
                None => {
                    e.local = Some(naive);
                    e.utc = Some(Utc.from_utc_datetime(&naive));
                }
            }
            break;
        }
    }
    e.orientation = f(tags, &["Orientation"]).map(|o| o as i32);
    let w = f(tags, &["ImageWidth", "ExifImageWidth", "SourceImageWidth"]).map(|v| v as i32);
    let h = f(tags, &["ImageHeight", "ExifImageHeight", "SourceImageHeight"]).map(|v| v as i32);
    let rotated = if is_video {
        matches!(f(tags, &["Rotation"]).map(|r| r as i32), Some(90) | Some(270))
    } else {
        matches!(e.orientation, Some(5..=8))
    };
    (e.width, e.height) = if rotated { (h, w) } else { (w, h) };
    e.duration_ms = f(tags, &["Duration", "MediaDuration", "TrackDuration"]).map(|d| (d * 1000.0) as i64);
    e.make = s(tags, &["Make", "AndroidMake"]);
    e.model = s(tags, &["Model", "AndroidModel"]);
    e.lens = s(tags, &["LensModel", "LensID", "Lens", "LensType"]);
    e.f_number = f(tags, &["FNumber", "Aperture"]);
    e.exposure = f(tags, &["ExposureTime", "ShutterSpeed"]).map(media::format_exposure);
    e.iso = f(tags, &["ISO", "RecommendedExposureIndex"]).map(|v| v as i32);
    e.focal_length = f(tags, &["FocalLength"]);
    e.lat = f(tags, &["GPSLatitude"]);
    e.lon = f(tags, &["GPSLongitude"]);
    e.alt = f(tags, &["GPSAltitude"]);
    if let (Some(lat), Some(lon)) = (e.lat.as_mut(), e.lon.as_mut()) {
        if s(tags, &["GPSLatitudeRef"]).is_some_and(|r| r.starts_with('S')) && *lat > 0.0 {
            *lat = -*lat;
        }
        if s(tags, &["GPSLongitudeRef"]).is_some_and(|r| r.starts_with('W')) && *lon > 0.0 {
            *lon = -*lon;
        }
    }
    if e.lat.is_none() {
        // QuickTime: "52.5 13.4 34" (with -n)
        if let Some(coords) = s(tags, &["GPSCoordinates", "LocationInformation"]) {
            let parts: Vec<f64> = coords.split([' ', ',']).filter_map(|p| p.parse().ok()).collect();
            if parts.len() >= 2 {
                (e.lat, e.lon) = (Some(parts[0]), Some(parts[1]));
                e.alt = parts.get(2).copied();
            }
        }
    }
    if matches!((e.lat, e.lon), (Some(a), Some(b)) if (a == 0.0 && b == 0.0) || !(-90.0..=90.0).contains(&a) || !(-180.0..=180.0).contains(&b))
    {
        (e.lat, e.lon, e.alt) = (None, None, None);
    }
    e.projection = s(tags, &["ProjectionType"]).map(|p| p.to_ascii_uppercase()).filter(|p| p == "EQUIRECTANGULAR");
    let aspect = match (e.width, e.height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => w as f64 / h as f64,
        _ => 1.0,
    };
    e.is_panorama = e.projection.is_some()
        || tags.contains_key("FullPanoWidthPixels")
        || (!is_video && !(0.4..2.5).contains(&aspect));
    e.description = s(tags, &["Description", "ImageDescription", "Caption-Abstract", "UserComment"])
        .filter(|d| !d.chars().all(|c| c == ' ' || c == '\0'));
    e.rating = f(tags, &["Rating"]).map(|r| r.clamp(0.0, 5.0) as i16);
    e.content_id = s(tags, &["ContentIdentifier", "MediaGroupUUID"]);
    e.motion = f(tags, &["MotionPhoto", "MicroVideo"]).map(|v| v >= 1.0).unwrap_or(false)
        || s(tags, &["EmbeddedVideoType"]).is_some();
    e.micro_video_offset = f(tags, &["MicroVideoOffset"]).map(|v| v as usize);
    for key in ["TagsList", "HierarchicalSubject", "Keywords", "Subject"] {
        match tags.get(key) {
            Some(Value::Array(a)) => e.keywords.extend(a.iter().filter_map(|v| v.as_str().map(String::from))),
            Some(Value::String(v)) => e.keywords.push(v.clone()),
            _ => {}
        }
        if !e.keywords.is_empty() {
            break;
        }
    }
    e.keywords = e
        .keywords
        .iter()
        .map(|k| k.replace('|', "/").trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    e.mime = s(tags, &["MIMEType"]);
    e
}

/// Fallback when exiftool is unavailable: kamadak-exif (images) / ffprobe (videos).
async fn fallback_tags(path: &Path, is_video: bool) -> Tags {
    let mut tags = Tags::new();
    if is_video {
        if let Ok(p) = media::ffprobe(path).await {
            if let Some(d) = p.duration {
                tags.insert("Duration".into(), d.into());
            }
            if let (Some(w), Some(h)) = (p.width, p.height) {
                tags.insert("ImageWidth".into(), w.into());
                tags.insert("ImageHeight".into(), h.into());
            }
            tags.insert("Rotation".into(), p.rotation.into());
        }
        return tags;
    }
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let Ok(file) = std::fs::File::open(&path) else { return tags };
        let mut reader = std::io::BufReader::new(file);
        let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else { return tags };
        use exif::{In, Tag};
        let str_of = |t: Tag| exif.get_field(t, In::PRIMARY).map(|f| f.display_value().to_string().trim_matches('"').to_string());
        if let Some(v) = str_of(Tag::DateTimeOriginal) {
            tags.insert("DateTimeOriginal".into(), v.replace('-', ":").into());
        }
        if let Some(v) = str_of(Tag::OffsetTimeOriginal) {
            tags.insert("OffsetTimeOriginal".into(), v.into());
        }
        for (t, k) in [(Tag::Make, "Make"), (Tag::Model, "Model"), (Tag::LensModel, "LensModel")] {
            if let Some(v) = str_of(t) {
                tags.insert(k.into(), v.into());
            }
        }
        let num = |t: Tag| -> Option<f64> {
            let f = exif.get_field(t, In::PRIMARY)?;
            match &f.value {
                exif::Value::Rational(r) if !r.is_empty() => Some(r[0].to_f64()),
                exif::Value::Short(s) if !s.is_empty() => Some(s[0] as f64),
                exif::Value::Long(l) if !l.is_empty() => Some(l[0] as f64),
                _ => f.value.get_uint(0).map(|v| v as f64),
            }
        };
        let dms = |t: Tag| -> Option<f64> {
            match &exif.get_field(t, In::PRIMARY)?.value {
                exif::Value::Rational(r) if r.len() >= 3 => {
                    Some(r[0].to_f64() + r[1].to_f64() / 60.0 + r[2].to_f64() / 3600.0)
                }
                _ => None,
            }
        };
        for (t, k) in [
            (Tag::Orientation, "Orientation"),
            (Tag::FNumber, "FNumber"),
            (Tag::ExposureTime, "ExposureTime"),
            (Tag::PhotographicSensitivity, "ISO"),
            (Tag::FocalLength, "FocalLength"),
            (Tag::PixelXDimension, "ExifImageWidth"),
            (Tag::PixelYDimension, "ExifImageHeight"),
        ] {
            if let Some(v) = num(t) {
                tags.insert(k.into(), v.into());
            }
        }
        if let (Some(lat), Some(lon)) = (dms(Tag::GPSLatitude), dms(Tag::GPSLongitude)) {
            tags.insert("GPSLatitude".into(), lat.into());
            tags.insert("GPSLongitude".into(), lon.into());
            if let Some(r) = str_of(Tag::GPSLatitudeRef) {
                tags.insert("GPSLatitudeRef".into(), r.into());
            }
            if let Some(r) = str_of(Tag::GPSLongitudeRef) {
                tags.insert("GPSLongitudeRef".into(), r.into());
            }
        }
        tags
    })
    .await
    .unwrap_or_default()
}

pub async fn read_tags(path: &Path, is_video: bool) -> Tags {
    match media::exiftool(path).await {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!("exiftool unavailable/failed ({e}), using fallback");
            fallback_tags(path, is_video).await
        }
    }
}

fn sidecar_for(asset: &AssetRow) -> Option<PathBuf> {
    if let Some(p) = &asset.sidecar_path {
        return Some(PathBuf::from(p));
    }
    let original = PathBuf::from(&asset.original_path);
    let a = PathBuf::from(format!("{}.xmp", asset.original_path));
    let b = original.with_extension("xmp");
    [a, b].into_iter().find(|p| p.exists())
}

pub async fn run(state: &SharedState, asset_id: Uuid) -> anyhow::Result<()> {
    let asset = get_asset(&state.db, asset_id).await?;
    let path = PathBuf::from(&asset.original_path);
    if !path.exists() {
        sqlx::query("UPDATE assets SET is_offline = TRUE, updated_at = now() WHERE id = $1")
            .bind(asset_id)
            .execute(&state.db)
            .await?;
        return Ok(());
    }
    let mut tags = read_tags(&path, asset.is_video()).await;
    if let Some(sidecar) = sidecar_for(&asset) {
        if let Ok(side) = media::exiftool(&sidecar).await {
            for (k, v) in side {
                if !matches!(k.as_str(), "SourceFile" | "FileName" | "Directory" | "FileSize" | "MIMEType" | "FileType") {
                    tags.insert(k, v);
                }
            }
        }
    }
    let mut e = extract(&tags, asset.is_video());
    if asset.is_video() && (e.duration_ms.is_none() || e.width.is_none()) {
        if let Ok(p) = media::ffprobe(&path).await {
            e.duration_ms = e.duration_ms.or(p.duration.map(|d| (d * 1000.0) as i64));
            if e.width.is_none() {
                let swap = p.rotation == 90 || p.rotation == 270;
                (e.width, e.height) = if swap { (p.height, p.width) } else { (p.width, p.height) };
            }
        }
    }

    let (city, region, country) = match (e.lat, e.lon) {
        (Some(lat), Some(lon)) if state.sys().reverse_geocoding.enabled => match state.geocoder.lookup(lat, lon) {
            Some(p) => (Some(p.city), p.state, p.country),
            None => (None, None, None),
        },
        _ => (None, None, None),
    };

    sqlx::query(
        "INSERT INTO exif (asset_id, make, model, lens_model, f_number, exposure_time, iso, focal_length,
            latitude, longitude, altitude, city, state, country, orientation, date_time_original, time_zone,
            projection_type, description)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)
         ON CONFLICT (asset_id) DO UPDATE SET make=$2, model=$3, lens_model=$4, f_number=$5, exposure_time=$6,
            iso=$7, focal_length=$8, latitude=$9, longitude=$10, altitude=$11, city=$12, state=$13, country=$14,
            orientation=$15, date_time_original=$16, time_zone=$17, projection_type=$18, description=$19",
    )
    .bind(asset_id)
    .bind(&e.make)
    .bind(&e.model)
    .bind(&e.lens)
    .bind(e.f_number)
    .bind(&e.exposure)
    .bind(e.iso)
    .bind(e.focal_length)
    .bind(e.lat)
    .bind(e.lon)
    .bind(e.alt)
    .bind(&city)
    .bind(&region)
    .bind(&country)
    .bind(e.orientation)
    .bind(e.utc)
    .bind(&e.time_zone)
    .bind(&e.projection)
    .bind(&e.description)
    .execute(&state.db)
    .await?;

    let local = e.local.map(|n| Utc.from_utc_datetime(&n));
    sqlx::query(
        "UPDATE assets SET
            local_date_time = COALESCE($2, local_date_time),
            width = COALESCE($3, width), height = COALESCE($4, height),
            duration_ms = COALESCE($5, duration_ms),
            projection_type = $6, is_panorama = $7,
            description = CASE WHEN description = '' THEN COALESCE($8, '') ELSE description END,
            rating = COALESCE(rating, $9),
            content_identifier = $10,
            original_mime_type = COALESCE($11, original_mime_type),
            metadata_at = now(), updated_at = now(), is_offline = FALSE
         WHERE id = $1",
    )
    .bind(asset_id)
    .bind(local)
    .bind(e.width)
    .bind(e.height)
    .bind(e.duration_ms)
    .bind(&e.projection)
    .bind(e.is_panorama)
    .bind(&e.description)
    .bind(e.rating)
    .bind(&e.content_id)
    .bind(&e.mime)
    .execute(&state.db)
    .await?;

    import_keywords(state, &asset, &e.keywords).await?;
    link_live_photo(state, &asset, e.content_id.as_deref()).await?;
    if !asset.is_video() && asset.live_photo_video_id.is_none() && (e.motion || looks_like_motion(&asset.original_file_name)) {
        if let Err(err) = extract_motion_video(state, &asset, e.micro_video_offset).await {
            tracing::warn!("motion photo extraction failed for {asset_id}: {err:#}");
        }
    }

    state.jobs.enqueue_asset("thumbnails", asset_id).await?;
    if asset.is_video() {
        state.jobs.enqueue_asset("video", asset_id).await?;
    }
    Ok(())
}

fn looks_like_motion(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    n.starts_with("MVIMG_") || n.contains(".MP.") || n.ends_with(".MP.JPG")
}

async fn import_keywords(state: &SharedState, asset: &AssetRow, keywords: &[String]) -> anyhow::Result<()> {
    for kw in keywords.iter().take(50) {
        let tag_id = crate::routes::tags::upsert_tag(state, asset.owner_id, kw, None).await?;
        sqlx::query("INSERT INTO tag_assets (tag_id, asset_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
            .bind(tag_id)
            .bind(asset.id)
            .execute(&state.db)
            .await?;
    }
    Ok(())
}

/// Pairs Apple Live Photo stills and videos by their content identifier.
async fn link_live_photo(state: &SharedState, asset: &AssetRow, content_id: Option<&str>) -> anyhow::Result<()> {
    let Some(cid) = content_id else { return Ok(()) };
    let other_type = if asset.is_video() { "IMAGE" } else { "VIDEO" };
    let other: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM assets WHERE owner_id = $1 AND content_identifier = $2 AND asset_type = $3 AND id <> $4
         AND deleted_at IS NULL LIMIT 1",
    )
    .bind(asset.owner_id)
    .bind(cid)
    .bind(other_type)
    .bind(asset.id)
    .fetch_optional(&state.db)
    .await?;
    let Some(other) = other else { return Ok(()) };
    let (image, video) = if asset.is_video() { (other, asset.id) } else { (asset.id, other) };
    link_pair(state, image, video).await
}

pub async fn link_pair(state: &SharedState, image: Uuid, video: Uuid) -> anyhow::Result<()> {
    sqlx::query("UPDATE assets SET live_photo_video_id = $2, updated_at = now() WHERE id = $1")
        .bind(image)
        .bind(video)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE assets SET visibility = 'hidden', updated_at = now() WHERE id = $1")
        .bind(video)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Extracts the MP4 embedded in Google/Samsung motion photos into a hidden video asset.
async fn extract_motion_video(state: &SharedState, asset: &AssetRow, offset: Option<usize>) -> anyhow::Result<()> {
    let bytes = tokio::fs::read(&asset.original_path).await?;
    let Some(start) = media::find_embedded_video(&bytes, offset) else { return Ok(()) };
    let video = &bytes[start..];
    if video.len() < 1024 {
        return Ok(());
    }
    let checksum = hex::encode(Sha1::digest(video));
    let id = Uuid::new_v4();
    let path = state
        .storage
        .root
        .join("library")
        .join(asset.owner_id.to_string())
        .join("motion")
        .join(format!("{id}.mp4"));
    tokio::fs::create_dir_all(path.parent().expect("parent")).await?;
    tokio::fs::write(&path, video).await?;
    let stem = asset.original_file_name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&asset.original_file_name);
    let inserted = sqlx::query(
        "INSERT INTO assets (id, owner_id, asset_type, original_path, original_file_name, original_mime_type, checksum,
            file_size, file_created_at, file_modified_at, local_date_time, visibility)
         VALUES ($1,$2,'VIDEO',$3,$4,'video/mp4',$5,$6,$7,$8,$9,'hidden')
         ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(asset.owner_id)
    .bind(path.to_string_lossy().to_string())
    .bind(format!("{stem}.mp4"))
    .bind(&checksum)
    .bind(video.len() as i64)
    .bind(asset.file_created_at)
    .bind(asset.file_modified_at)
    .bind(asset.local_date_time)
    .execute(&state.db)
    .await?;
    if inserted.rows_affected() == 0 {
        let _ = tokio::fs::remove_file(&path).await;
        return Ok(());
    }
    link_pair(state, asset.id, id).await?;
    sqlx::query("UPDATE assets SET is_motion_photo = TRUE WHERE id = $1").bind(asset.id).execute(&state.db).await?;
    state.jobs.enqueue_asset("metadata", id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_dates_with_offsets() {
        let (n, o) = parse_exif_date("2024:05:01 12:30:00.123+02:00").unwrap();
        assert_eq!(n.to_string(), "2024-05-01 12:30:00");
        assert_eq!(o.unwrap().local_minus_utc(), 7200);
        assert!(parse_exif_date("0000:00:00 00:00:00").is_none());
    }

    #[test]
    fn extracts_core_fields() {
        let tags: Tags = serde_json::from_value(json!({
            "DateTimeOriginal": "2023:08:15 18:00:00", "OffsetTimeOriginal": "+02:00",
            "Make": "Apple", "Model": "iPhone 15", "FNumber": 1.8, "ExposureTime": 0.008, "ISO": 50,
            "GPSLatitude": 48.1, "GPSLatitudeRef": "N", "GPSLongitude": 11.5, "GPSLongitudeRef": "E",
            "ImageWidth": 4032, "ImageHeight": 3024, "Orientation": 6, "ContentIdentifier": "ABC",
            "Keywords": ["Urlaub", "Familie|Kinder"]
        }))
        .unwrap();
        let e = extract(&tags, false);
        assert_eq!(e.utc.unwrap().to_rfc3339(), "2023-08-15T16:00:00+00:00");
        assert_eq!((e.width, e.height), (Some(3024), Some(4032)));
        assert_eq!(e.exposure.as_deref(), Some("1/125"));
        assert_eq!(e.content_id.as_deref(), Some("ABC"));
        assert_eq!(e.keywords, vec!["Urlaub", "Familie/Kinder"]);
        assert!(!e.is_panorama);
    }

    #[test]
    fn detects_360_and_south_west() {
        let tags: Tags = serde_json::from_value(json!({
            "ProjectionType": "equirectangular", "ImageWidth": 6000, "ImageHeight": 3000,
            "GPSLatitude": 33.9, "GPSLatitudeRef": "S", "GPSLongitude": 70.6, "GPSLongitudeRef": "W"
        }))
        .unwrap();
        let e = extract(&tags, false);
        assert_eq!(e.projection.as_deref(), Some("EQUIRECTANGULAR"));
        assert!(e.is_panorama);
        assert_eq!((e.lat, e.lon), (Some(-33.9), Some(-70.6)));
    }
}
