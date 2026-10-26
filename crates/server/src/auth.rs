//! Authentication: password hashing, opaque tokens, request principals.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::state::SharedState;

pub const SESSION_COOKIE: &str = "ph_session";

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub password_hash: Option<String>,
    pub is_admin: bool,
    pub oauth_id: Option<String>,
    pub quota_bytes: Option<i64>,
    pub pin_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SharedLinkRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub key: String,
    pub link_type: String,
    pub album_id: Option<Uuid>,
    pub description: String,
    pub password_hash: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub allow_download: bool,
    pub allow_upload: bool,
    pub show_metadata: bool,
    pub created_at: DateTime<Utc>,
}

/// Who is making the request.
#[derive(Debug, Clone)]
pub enum Principal {
    User { user: UserRow, session_id: Option<Uuid>, unlocked: bool },
    Share { link: SharedLinkRow },
    Cast { asset_id: Uuid },
}

/// Extractor that accepts any principal (user, share link, cast token).
#[derive(Debug, Clone)]
pub struct AnyAuth(pub Principal);

/// Extractor that requires a logged in user.
#[derive(Debug, Clone)]
pub struct Auth {
    pub user: UserRow,
    pub session_id: Option<Uuid>,
    pub unlocked: bool,
}

/// Extractor that requires an admin user.
#[derive(Debug, Clone)]
pub struct AdminAuth(pub Auth);

impl Auth {
    pub fn id(&self) -> Uuid {
        self.user.id
    }
    pub fn require_unlocked(&self) -> AppResult<()> {
        if self.unlocked { Ok(()) } else { Err(AppError::Forbidden("locked".into())) }
    }
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("hash: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

pub fn random_token(bytes: usize) -> String {
    let buf: Vec<u8> = (0..bytes).map(|_| rand::random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(buf)
}

pub fn sha256_hex(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

pub fn sign(secret: &[u8], message: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("hmac key");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

pub fn cast_token(secret: &[u8], asset_id: Uuid, expires: DateTime<Utc>) -> String {
    let exp = expires.timestamp();
    format!("{exp}.{}", sign(secret, &format!("cast:{asset_id}:{exp}")))
}

fn verify_cast_token(token: &str) -> Option<(i64, String)> {
    let (exp, sig) = token.split_once('.')?;
    let exp: i64 = exp.parse().ok()?;
    if exp < Utc::now().timestamp() {
        return None;
    }
    Some((exp, sig.to_string()))
}

pub fn share_token(secret: &[u8], link: &SharedLinkRow) -> String {
    sign(secret, &format!("share:{}:{}", link.id, link.password_hash.as_deref().unwrap_or("")))
}

/// Creates a login session and returns the plain token.
pub async fn create_session(
    state: &SharedState,
    user_id: Uuid,
    device_type: Option<String>,
    device_name: Option<String>,
) -> AppResult<String> {
    let token = random_token(32);
    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, device_type, device_name) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(sha256_hex(&token))
    .bind(device_type)
    .bind(device_name)
    .execute(&state.db)
    .await?;
    Ok(token)
}

fn cookie_value(parts: &Parts, name: &str) -> Option<String> {
    parts
        .headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

fn query_params(parts: &Parts) -> Vec<(String, String)> {
    parts
        .uri
        .query()
        .map(|q| url::form_urlencoded::parse(q.as_bytes()).into_owned().collect())
        .unwrap_or_default()
}

pub fn bearer_token(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
        .map(|v| v.trim().to_string())
        .or_else(|| cookie_value(parts, SESSION_COOKIE))
}

async fn user_from_session(state: &SharedState, token: &str) -> AppResult<Option<Auth>> {
    #[derive(FromRow)]
    struct S {
        id: Uuid,
        user_id: Uuid,
        unlocked_until: Option<DateTime<Utc>>,
        last_used_at: DateTime<Utc>,
    }
    let session: Option<S> = sqlx::query_as(
        "SELECT id, user_id, unlocked_until, last_used_at FROM sessions WHERE token_hash = $1",
    )
    .bind(sha256_hex(token))
    .fetch_optional(&state.db)
    .await?;
    let Some(session) = session else { return Ok(None) };
    if Utc::now() - session.last_used_at > chrono::Duration::minutes(5) {
        sqlx::query("UPDATE sessions SET last_used_at = now() WHERE id = $1")
            .bind(session.id)
            .execute(&state.db)
            .await?;
    }
    let user: UserRow = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(session.user_id)
        .fetch_one(&state.db)
        .await?;
    let unlocked = session.unlocked_until.map(|t| t > Utc::now()).unwrap_or(false);
    Ok(Some(Auth { user, session_id: Some(session.id), unlocked }))
}

async fn user_from_api_key(state: &SharedState, key: &str) -> AppResult<Option<Auth>> {
    let user: Option<UserRow> = sqlx::query_as(
        "SELECT u.* FROM users u JOIN api_keys k ON k.user_id = u.id WHERE k.key_hash = $1",
    )
    .bind(sha256_hex(key))
    .fetch_optional(&state.db)
    .await?;
    // API keys never unlock the locked folder.
    Ok(user.map(|user| Auth { user, session_id: None, unlocked: false }))
}

pub async fn resolve_principal(parts: &Parts, state: &SharedState) -> AppResult<Option<Principal>> {
    let params = query_params(parts);
    let param = |name: &str| params.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    if let Some(key) = param("key") {
        let link: Option<SharedLinkRow> = sqlx::query_as("SELECT * FROM shared_links WHERE key = $1")
            .bind(&key)
            .fetch_optional(&state.db)
            .await?;
        let Some(link) = link else { return Err(AppError::Unauthorized("invalid share key".into())) };
        if link.expires_at.map(|e| e < Utc::now()).unwrap_or(false) {
            return Err(AppError::Unauthorized("share link expired".into()));
        }
        if link.password_hash.is_some() {
            let expected = share_token(&state.secret, &link);
            if param("shareToken").as_deref() != Some(expected.as_str()) {
                return Err(AppError::Unauthorized("password_required".into()));
            }
        }
        return Ok(Some(Principal::Share { link }));
    }
    if let Some(token) = param("castToken") {
        if let Some((exp, sig)) = verify_cast_token(&token) {
            // The asset id is part of the path: /api/assets/{id}/...
            // Nested routers see the path without `/api`; prefer the original URI.
            let path = parts
                .extensions
                .get::<axum::extract::OriginalUri>()
                .map(|u| u.0.path().to_string())
                .unwrap_or_else(|| parts.uri.path().to_string());
            let asset_id = path
                .strip_prefix("/api/assets/")
                .or_else(|| path.strip_prefix("/assets/"))
                .and_then(|rest| rest.split('/').next())
                .and_then(|id| Uuid::parse_str(id).ok());
            if let Some(asset_id) = asset_id {
                if sign(&state.secret, &format!("cast:{asset_id}:{exp}")) == sig {
                    return Ok(Some(Principal::Cast { asset_id }));
                }
            }
        }
        return Err(AppError::Unauthorized("invalid cast token".into()));
    }
    if let Some(key) = parts.headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        return Ok(user_from_api_key(state, key)
            .await?
            .map(|a| Principal::User { user: a.user, session_id: None, unlocked: false }));
    }
    if let Some(token) = bearer_token(parts) {
        if let Some(a) = user_from_session(state, &token).await? {
            return Ok(Some(Principal::User { user: a.user, session_id: a.session_id, unlocked: a.unlocked }));
        }
    }
    Ok(None)
}

impl FromRequestParts<SharedState> for AnyAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self, Self::Rejection> {
        resolve_principal(parts, state)
            .await?
            .map(AnyAuth)
            .ok_or_else(|| AppError::Unauthorized("authentication required".into()))
    }
}

impl FromRequestParts<SharedState> for Auth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self, Self::Rejection> {
        match resolve_principal(parts, state).await? {
            Some(Principal::User { user, session_id, unlocked }) => Ok(Auth { user, session_id, unlocked }),
            _ => Err(AppError::Unauthorized("authentication required".into())),
        }
    }
}

impl FromRequestParts<SharedState> for AdminAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self, Self::Rejection> {
        let auth = Auth::from_request_parts(parts, state).await?;
        if !auth.user.is_admin {
            return Err(AppError::forbidden());
        }
        Ok(AdminAuth(auth))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let h = hash_password("secret123").unwrap();
        assert!(verify_password("secret123", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn cast_token_roundtrip() {
        let secret = b"0123456789abcdef";
        let id = Uuid::new_v4();
        let tok = cast_token(secret, id, Utc::now() + chrono::Duration::hours(1));
        let (exp, sig) = verify_cast_token(&tok).unwrap();
        assert_eq!(sign(secret, &format!("cast:{id}:{exp}")), sig);
        let expired = cast_token(secret, id, Utc::now() - chrono::Duration::hours(1));
        assert!(verify_cast_token(&expired).is_none());
    }
}
