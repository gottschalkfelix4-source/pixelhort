use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use pixelhort_common::{UserDto, UserSummary};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth::{AdminAuth, Auth, UserRow, hash_password, random_token, sha256_hex, verify_password};
use crate::error::{AppError, AppResult};
use crate::model::user_dto;
use crate::routes::auth::validate_password;
use crate::state::SharedState;
use crate::sysconfig::merge_json;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/users", get(list_users))
        .route("/users/me", get(me).put(update_me))
        .route("/users/me/pin", put(set_pin).delete(reset_pin))
        .route("/users/me/preferences", get(get_preferences).put(put_preferences))
        .route("/api-keys", get(list_keys).post(create_key))
        .route("/api-keys/{id}", delete(delete_key))
        .route("/admin/users", get(admin_list).post(admin_create))
        .route("/admin/users/{id}", put(admin_update).delete(admin_delete))
}

async fn me(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<UserDto>> {
    Ok(Json(user_dto(&state.db, &auth.user).await?))
}

async fn list_users(State(state): State<SharedState>, _auth: Auth) -> AppResult<Json<Vec<UserSummary>>> {
    let rows: Vec<(Uuid, String, String)> =
        sqlx::query_as("SELECT id, name, email FROM users ORDER BY name").fetch_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(|(id, name, email)| UserSummary { id, name, email }).collect()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateMe {
    name: Option<String>,
    email: Option<String>,
    password: Option<String>,
    current_password: Option<String>,
}

async fn update_me(State(state): State<SharedState>, auth: Auth, Json(req): Json<UpdateMe>) -> AppResult<Json<UserDto>> {
    if let Some(pw) = &req.password {
        validate_password(pw)?;
        if let Some(hash) = &auth.user.password_hash {
            let ok = req.current_password.as_deref().map(|c| verify_password(c, hash)).unwrap_or(false);
            if !ok {
                return Err(AppError::bad("current password is wrong"));
            }
        }
    }
    let user: UserRow = sqlx::query_as(
        "UPDATE users SET name = COALESCE($2, name), email = COALESCE($3, email),
            password_hash = COALESCE($4, password_hash), updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(auth.id())
    .bind(req.name.map(|n| n.trim().to_string()))
    .bind(req.email.map(|e| e.trim().to_lowercase()))
    .bind(req.password.as_deref().map(hash_password).transpose()?)
    .fetch_one(&state.db)
    .await?;
    if req.password.is_some() {
        // Invalidate all other sessions after a password change.
        sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id IS DISTINCT FROM $2")
            .bind(auth.id())
            .bind(auth.session_id)
            .execute(&state.db)
            .await?;
    }
    Ok(Json(user_dto(&state.db, &user).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinBody {
    pin: String,
    current_pin: Option<String>,
}

async fn set_pin(State(state): State<SharedState>, auth: Auth, Json(req): Json<PinBody>) -> AppResult<StatusCode> {
    if !(4..=8).contains(&req.pin.len()) || !req.pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::bad("PIN must be 4-8 digits"));
    }
    if let Some(hash) = &auth.user.pin_hash {
        if !req.current_pin.as_deref().map(|p| verify_password(p, hash)).unwrap_or(false) {
            return Err(AppError::bad("current PIN is wrong"));
        }
    }
    sqlx::query("UPDATE users SET pin_hash = $2, updated_at = now() WHERE id = $1")
        .bind(auth.id())
        .bind(hash_password(&req.pin)?)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetPinBody {
    password: Option<String>,
}

/// Removes the PIN after re-authenticating with the account password (forgotten PIN).
/// Locked assets stay locked; a new PIN has to be set to open the folder again.
async fn reset_pin(State(state): State<SharedState>, auth: Auth, Json(req): Json<ResetPinBody>) -> AppResult<StatusCode> {
    if let Some(hash) = &auth.user.password_hash {
        if !req.password.as_deref().map(|p| verify_password(p, hash)).unwrap_or(false) {
            return Err(AppError::bad("password is wrong"));
        }
    }
    sqlx::query("UPDATE users SET pin_hash = NULL, updated_at = now() WHERE id = $1").bind(auth.id()).execute(&state.db).await?;
    sqlx::query("UPDATE sessions SET unlocked_until = NULL WHERE user_id = $1").bind(auth.id()).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Per-user preferences. Unknown keys are dropped, missing keys take the defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserPreferences {
    pub memories: Feature,
    pub people: Feature,
    pub tags: Feature,
    pub ratings: Feature,
    pub shared_links: Feature,
    pub download: DownloadPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Feature {
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DownloadPreferences {
    /// Also download the video part of Live/Motion Photos.
    pub include_live_video: bool,
}

impl Default for Feature {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            memories: Feature::default(),
            people: Feature::default(),
            tags: Feature::default(),
            ratings: Feature::default(),
            shared_links: Feature::default(),
            download: DownloadPreferences::default(),
        }
    }
}

fn preferences_from(stored: Value) -> UserPreferences {
    let mut value = serde_json::to_value(UserPreferences::default()).unwrap_or_default();
    merge_json(&mut value, stored);
    serde_json::from_value(value).unwrap_or_default()
}

async fn get_preferences(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<UserPreferences>> {
    let stored: Value =
        sqlx::query_scalar("SELECT preferences FROM users WHERE id = $1").bind(auth.id()).fetch_one(&state.db).await?;
    Ok(Json(preferences_from(stored)))
}

async fn put_preferences(State(state): State<SharedState>, auth: Auth, Json(patch): Json<Value>) -> AppResult<Json<UserPreferences>> {
    if !patch.is_object() {
        return Err(AppError::bad("preferences must be an object"));
    }
    let stored: Value =
        sqlx::query_scalar("SELECT preferences FROM users WHERE id = $1").bind(auth.id()).fetch_one(&state.db).await?;
    let mut value = serde_json::to_value(preferences_from(stored)).map_err(|e| AppError::Internal(e.into()))?;
    merge_json(&mut value, patch);
    let prefs: UserPreferences =
        serde_json::from_value(value).map_err(|e| AppError::bad(format!("invalid preferences: {e}")))?;
    sqlx::query("UPDATE users SET preferences = $2, updated_at = now() WHERE id = $1")
        .bind(auth.id())
        .bind(serde_json::to_value(&prefs).map_err(|e| AppError::Internal(e.into()))?)
        .execute(&state.db)
        .await?;
    Ok(Json(prefs))
}

async fn list_keys(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Value>> {
    let rows: Vec<(Uuid, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT id, name, created_at FROM api_keys WHERE user_id = $1 ORDER BY created_at")
            .bind(auth.id())
            .fetch_all(&state.db)
            .await?;
    Ok(Json(json!(rows.into_iter().map(|(id, name, c)| json!({"id": id, "name": name, "createdAt": c})).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
struct NameBody {
    name: String,
}

async fn create_key(State(state): State<SharedState>, auth: Auth, Json(req): Json<NameBody>) -> AppResult<Json<Value>> {
    let secret = format!("phk_{}", random_token(32));
    let id = Uuid::new_v4();
    let created: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "INSERT INTO api_keys (id, user_id, name, key_hash) VALUES ($1,$2,$3,$4) RETURNING created_at",
    )
    .bind(id)
    .bind(auth.id())
    .bind(req.name.trim())
    .bind(sha256_hex(&secret))
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({"id": id, "name": req.name.trim(), "createdAt": created, "secret": secret})))
}

async fn delete_key(State(state): State<SharedState>, auth: Auth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2").bind(id).bind(auth.id()).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn admin_list(State(state): State<SharedState>, _a: AdminAuth) -> AppResult<Json<Vec<UserDto>>> {
    let users: Vec<UserRow> = sqlx::query_as("SELECT * FROM users ORDER BY created_at").fetch_all(&state.db).await?;
    let mut out = Vec::new();
    for u in &users {
        out.push(user_dto(&state.db, u).await?);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminCreate {
    email: String,
    password: String,
    name: String,
    #[serde(default)]
    is_admin: bool,
    /// Missing: server default quota; `null`: unlimited.
    #[serde(default, with = "crate::model::double_option")]
    quota_bytes: Option<Option<i64>>,
}

async fn admin_create(State(state): State<SharedState>, _a: AdminAuth, Json(req): Json<AdminCreate>) -> AppResult<Json<UserDto>> {
    validate_password(&req.password)?;
    let user: UserRow = sqlx::query_as(
        "INSERT INTO users (id, email, name, password_hash, is_admin, quota_bytes) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(req.email.trim().to_lowercase())
    .bind(req.name.trim())
    .bind(hash_password(&req.password)?)
    .bind(req.is_admin)
    .bind(req.quota_bytes.unwrap_or(state.sys().user.default_quota_bytes))
    .fetch_one(&state.db)
    .await?;
    Ok(Json(user_dto(&state.db, &user).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminUpdate {
    name: Option<String>,
    email: Option<String>,
    password: Option<String>,
    is_admin: Option<bool>,
    #[serde(default, with = "crate::model::double_option")]
    quota_bytes: Option<Option<i64>>,
}

async fn admin_update(
    State(state): State<SharedState>,
    AdminAuth(admin): AdminAuth,
    Path(id): Path<Uuid>,
    Json(req): Json<AdminUpdate>,
) -> AppResult<Json<UserDto>> {
    if id == admin.id() && req.is_admin == Some(false) {
        return Err(AppError::bad("you cannot remove your own admin rights"));
    }
    if let Some(pw) = &req.password {
        validate_password(pw)?;
    }
    let user: UserRow = sqlx::query_as(
        "UPDATE users SET name = COALESCE($2, name), email = COALESCE($3, email),
            password_hash = COALESCE($4, password_hash), is_admin = COALESCE($5, is_admin),
            quota_bytes = CASE WHEN $6 THEN $7 ELSE quota_bytes END, updated_at = now()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(req.name)
    .bind(req.email.map(|e| e.trim().to_lowercase()))
    .bind(req.password.as_deref().map(hash_password).transpose()?)
    .bind(req.is_admin)
    .bind(req.quota_bytes.is_some())
    .bind(req.quota_bytes.flatten())
    .fetch_one(&state.db)
    .await?;
    Ok(Json(user_dto(&state.db, &user).await?))
}

async fn admin_delete(State(state): State<SharedState>, AdminAuth(admin): AdminAuth, Path(id): Path<Uuid>) -> AppResult<StatusCode> {
    if id == admin.id() {
        return Err(AppError::bad("you cannot delete yourself"));
    }
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM assets WHERE owner_id = $1").bind(id).fetch_all(&state.db).await?;
    crate::routes::assets::delete_permanently(&state, &ids).await?;
    sqlx::query("DELETE FROM users WHERE id = $1").bind(id).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_merge_defaults() {
        let p = preferences_from(serde_json::json!({"memories": {"enabled": false}, "unknown": 1}));
        assert!(!p.memories.enabled);
        assert!(p.people.enabled);
        assert!(p.ratings.enabled);
        let p = preferences_from(serde_json::json!({}));
        assert!(p.memories.enabled);
    }
}
