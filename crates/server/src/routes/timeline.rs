//! Timeline buckets, sync delta, map markers and memories.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use pixelhort_common::{AssetDto, TimeBucket};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use crate::access::{partner_ids, require_album_role};
use crate::auth::Auth;
use crate::error::{AppError, AppResult};
use crate::model::{ASSET_COLUMNS, AssetRow, assets_to_dtos};
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/timeline/buckets", get(buckets))
        .route("/timeline/bucket", get(bucket))
        .route("/sync/delta", get(sync_delta))
        .route("/map/markers", get(map_markers))
        .route("/memories", get(memories))
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimelineQuery {
    pub visibility: Option<String>,
    pub is_favorite: Option<bool>,
    pub album_id: Option<Uuid>,
    pub person_id: Option<Uuid>,
    pub tag_id: Option<Uuid>,
    pub with_partners: Option<bool>,
    pub is_trashed: Option<bool>,
    pub order: Option<String>,
    pub time_bucket: Option<NaiveDate>,
}

/// Appends `FROM assets a WHERE ...` for the caller's timeline scope.
pub async fn scope<'a>(state: &SharedState, auth: &Auth, q: &'a TimelineQuery, qb: &mut QueryBuilder<'a, Postgres>) -> AppResult<()> {
    qb.push(" FROM assets a WHERE a.visibility <> 'hidden'");
    let me = auth.id();
    if let Some(album) = q.album_id {
        require_album_role(state, me, album, false).await?;
        qb.push(" AND a.id IN (SELECT asset_id FROM album_assets WHERE album_id = ").push_bind(album).push(")");
        qb.push(" AND a.visibility IN ('timeline', 'archive') AND a.deleted_at IS NULL");
    } else {
        let partners = if q.with_partners.unwrap_or(false) { partner_ids(state, me, true).await? } else { vec![] };
        let visibility = q.visibility.clone().unwrap_or_else(|| "timeline".into());
        if visibility == "locked" {
            auth.require_unlocked()?;
        }
        if q.is_trashed.unwrap_or(false) {
            qb.push(" AND a.owner_id = ").push_bind(me).push(" AND a.deleted_at IS NOT NULL");
            if !auth.unlocked {
                qb.push(" AND a.visibility <> 'locked'");
            }
        } else {
            qb.push(" AND a.deleted_at IS NULL AND a.visibility = ").push_bind(visibility.clone());
            if partners.is_empty() || visibility != "timeline" {
                qb.push(" AND a.owner_id = ").push_bind(me);
            } else {
                qb.push(" AND (a.owner_id = ").push_bind(me).push(" OR a.owner_id = ANY(").push_bind(partners).push("))");
            }
        }
    }
    if let Some(fav) = q.is_favorite {
        qb.push(" AND a.is_favorite = ").push_bind(fav);
    }
    if let Some(person) = q.person_id {
        qb.push(" AND EXISTS (SELECT 1 FROM faces f WHERE f.asset_id = a.id AND f.person_id = ").push_bind(person).push(")");
    }
    if let Some(tag) = q.tag_id {
        qb.push(
            " AND EXISTS (SELECT 1 FROM tag_assets ta JOIN tags t ON t.id = ta.tag_id, tags root
              WHERE root.id = ",
        )
        .push_bind(tag)
        .push(" AND root.user_id = ")
        .push_bind(me)
        .push(" AND ta.asset_id = a.id AND (t.id = root.id OR t.value LIKE root.value || '/%'))");
    }
    Ok(())
}

async fn buckets(State(state): State<SharedState>, auth: Auth, Query(q): Query<TimelineQuery>) -> AppResult<Json<Vec<TimeBucket>>> {
    let mut qb = QueryBuilder::new("SELECT date_trunc('month', a.local_date_time)::date AS bucket, COUNT(*)");
    scope(&state, &auth, &q, &mut qb).await?;
    let asc = q.order.as_deref() == Some("asc");
    qb.push(if asc { " GROUP BY bucket ORDER BY bucket ASC" } else { " GROUP BY bucket ORDER BY bucket DESC" });
    let rows: Vec<(NaiveDate, i64)> = qb.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(|(time_bucket, count)| TimeBucket { time_bucket, count }).collect()))
}

async fn bucket(State(state): State<SharedState>, auth: Auth, Query(q): Query<TimelineQuery>) -> AppResult<Json<Vec<AssetDto>>> {
    let month = q.time_bucket.ok_or_else(|| AppError::bad("timeBucket is required"))?;
    let start = month.with_day(1).ok_or_else(|| AppError::bad("invalid timeBucket"))?;
    let end = if start.month() == 12 {
        NaiveDate::from_ymd_opt(start.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1)
    }
    .ok_or_else(|| AppError::bad("invalid timeBucket"))?;
    let mut qb = QueryBuilder::new(format!("SELECT {ASSET_COLUMNS}"));
    scope(&state, &auth, &q, &mut qb).await?;
    qb.push(" AND a.local_date_time >= ").push_bind(start.and_hms_opt(0, 0, 0).expect("midnight").and_utc());
    qb.push(" AND a.local_date_time < ").push_bind(end.and_hms_opt(0, 0, 0).expect("midnight").and_utc());
    let asc = q.order.as_deref() == Some("asc");
    qb.push(if asc { " ORDER BY a.local_date_time ASC, a.id" } else { " ORDER BY a.local_date_time DESC, a.id" });
    let rows: Vec<AssetRow> = qb.build_query_as().fetch_all(&state.db).await?;
    Ok(Json(assets_to_dtos(&rows)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncQuery {
    updated_after: Option<DateTime<Utc>>,
    limit: Option<i64>,
}

async fn sync_delta(State(state): State<SharedState>, auth: Auth, Query(q): Query<SyncQuery>) -> AppResult<Json<Value>> {
    let after = q.updated_after.unwrap_or(DateTime::<Utc>::MIN_UTC);
    let limit = q.limit.unwrap_or(1000).clamp(1, 5000);
    let rows: Vec<AssetRow> = sqlx::query_as(&format!(
        "SELECT {ASSET_COLUMNS} FROM assets a WHERE a.owner_id = $1 AND a.updated_at > $2
         AND (a.visibility <> 'locked' OR $4) ORDER BY a.updated_at, a.id LIMIT $3"
    ))
    .bind(auth.id())
    .bind(after)
    .bind(limit + 1)
    .bind(auth.unlocked)
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 > limit;
    let rows: Vec<AssetRow> = rows.into_iter().take(limit as usize).collect();
    let cursor = rows.last().map(|r| r.updated_at).unwrap_or(after);
    let deleted: Vec<Uuid> = sqlx::query_scalar(
        "SELECT asset_id FROM asset_deletions WHERE owner_id = $1 AND deleted_at > $2 AND deleted_at <= $3",
    )
    .bind(auth.id())
    .bind(after)
    .bind(if has_more { cursor } else { Utc::now() })
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "upserted": assets_to_dtos(&rows),
        "deleted": deleted,
        "hasMore": has_more,
        "cursor": if has_more { cursor } else { rows.last().map(|r| r.updated_at).unwrap_or(after).max(after) },
    })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapQuery {
    is_favorite: Option<bool>,
    with_partners: Option<bool>,
    file_created_after: Option<DateTime<Utc>>,
    file_created_before: Option<DateTime<Utc>>,
}

async fn map_markers(State(state): State<SharedState>, auth: Auth, Query(q): Query<MapQuery>) -> AppResult<Json<Value>> {
    let mut owners = vec![auth.id()];
    if q.with_partners.unwrap_or(false) {
        owners.extend(partner_ids(&state, auth.id(), false).await?);
    }
    let rows: Vec<(Uuid, f64, f64, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, e.latitude, e.longitude, e.city, e.state, e.country FROM assets a JOIN exif e ON e.asset_id = a.id
         WHERE a.owner_id = ANY($1) AND a.deleted_at IS NULL AND a.visibility = 'timeline'
           AND e.latitude IS NOT NULL AND e.longitude IS NOT NULL
           AND ($2::bool IS NULL OR a.is_favorite = $2)
           AND ($3::timestamptz IS NULL OR a.local_date_time >= $3)
           AND ($4::timestamptz IS NULL OR a.local_date_time <= $4)
         ORDER BY a.local_date_time DESC",
    )
    .bind(&owners)
    .bind(q.is_favorite)
    .bind(q.file_created_after)
    .bind(q.file_created_before)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, lat, lon, city, st, country)| json!({
            "id": id, "lat": lat, "lon": lon, "city": city, "state": st, "country": country
        }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct MemoryQuery {
    date: Option<NaiveDate>,
}

async fn memories(State(state): State<SharedState>, auth: Auth, Query(q): Query<MemoryQuery>) -> AppResult<Json<Value>> {
    let date = q.date.unwrap_or_else(|| Utc::now().date_naive());
    let rows: Vec<AssetRow> = sqlx::query_as(&format!(
        "SELECT * FROM (
            SELECT {ASSET_COLUMNS}, ROW_NUMBER() OVER (PARTITION BY EXTRACT(YEAR FROM a.local_date_time) ORDER BY random()) AS rn
            FROM assets a
            WHERE a.owner_id = $1 AND a.deleted_at IS NULL AND a.visibility = 'timeline'
              AND EXTRACT(MONTH FROM a.local_date_time) = $2 AND EXTRACT(DAY FROM a.local_date_time) = $3
              AND EXTRACT(YEAR FROM a.local_date_time) < $4
         ) sub WHERE rn <= 30 ORDER BY local_date_time DESC"
    ))
    .bind(auth.id())
    .bind(date.month() as i32)
    .bind(date.day() as i32)
    .bind(date.year())
    .fetch_all(&state.db)
    .await?;
    let mut groups: Vec<(i32, Vec<AssetRow>)> = Vec::new();
    for r in rows {
        let year = r.local_date_time.year();
        match groups.last_mut() {
            Some((y, list)) if *y == year => list.push(r),
            _ => groups.push((year, vec![r])),
        }
    }
    let out: Vec<Value> = groups
        .into_iter()
        .map(|(year, mut list)| {
            list.sort_by_key(|a| a.local_date_time);
            let years_ago = date.year() - year;
            json!({
                "id": format!("on-this-day-{year}"),
                "type": "on_this_day",
                "yearsAgo": years_ago,
                "year": year,
                "title": if years_ago == 1 { "1 year ago".to_string() } else { format!("{years_ago} years ago") },
                "assets": assets_to_dtos(&list),
            })
        })
        .collect();
    Ok(Json(json!(out)))
}
