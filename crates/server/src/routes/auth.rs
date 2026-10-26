use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{Duration, Utc};
use pixelhort_common::{LoginRequest, LoginResponse};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{
    Auth, SESSION_COOKIE, UserRow, bearer_token, create_session, hash_password, random_token, sha256_hex,
    verify_password,
};
use crate::error::{AppError, AppResult};
use crate::model::user_dto;
use crate::state::SharedState;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/auth/status", get(status))
        .route("/auth/setup", post(setup))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/oauth/authorize", get(oauth_authorize))
        .route("/auth/oauth/callback", post(oauth_callback))
        .route("/auth/session", get(session_info))
        .route("/auth/session/unlock", post(unlock))
        .route("/auth/session/lock", post(lock))
}

async fn status(State(state): State<SharedState>) -> AppResult<Json<Value>> {
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&state.db).await?;
    let sys = state.sys();
    Ok(Json(json!({
        "setupRequired": users == 0,
        "oauthEnabled": sys.oauth.enabled,
        "oauthButtonText": sys.oauth.button_text,
        "passwordLoginEnabled": sys.password_login.enabled,
    })))
}

fn login_response(token: String, body: LoginResponse) -> Response {
    let cookie = format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000");
    let mut res = Json(body).into_response();
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        res.headers_mut().insert(header::SET_COOKIE, v);
    }
    res
}

#[derive(Deserialize)]
struct SetupRequest {
    email: String,
    password: String,
    name: String,
}

fn validate_email(email: &str) -> AppResult<String> {
    let e = email.trim().to_lowercase();
    if !e.contains('@') || e.len() < 3 {
        return Err(AppError::bad("invalid email"));
    }
    Ok(e)
}

pub fn validate_password(p: &str) -> AppResult<()> {
    if p.chars().count() < 8 {
        return Err(AppError::bad("password must be at least 8 characters"));
    }
    Ok(())
}

async fn setup(State(state): State<SharedState>, Json(req): Json<SetupRequest>) -> AppResult<Response> {
    let email = validate_email(&req.email)?;
    validate_password(&req.password)?;
    let mut tx = state.db.begin().await?;
    sqlx::query("LOCK TABLE users IN EXCLUSIVE MODE").execute(&mut *tx).await?;
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&mut *tx).await?;
    if users > 0 {
        return Err(AppError::bad("setup already completed"));
    }
    let user: UserRow = sqlx::query_as(
        "INSERT INTO users (id, email, name, password_hash, is_admin) VALUES ($1,$2,$3,$4,TRUE) RETURNING *",
    )
    .bind(Uuid::new_v4())
    .bind(&email)
    .bind(req.name.trim())
    .bind(hash_password(&req.password)?)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let token = create_session(&state, user.id, Some("web".into()), None).await?;
    let dto = user_dto(&state.db, &user).await?;
    Ok(login_response(token.clone(), LoginResponse { access_token: token, user: dto }))
}

async fn login(State(state): State<SharedState>, Json(req): Json<LoginRequest>) -> AppResult<Response> {
    if !state.sys().password_login.enabled {
        return Err(AppError::bad("password login is disabled"));
    }
    let user: Option<UserRow> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(req.email.trim().to_lowercase())
        .fetch_optional(&state.db)
        .await?;
    let user = user
        .filter(|u| u.password_hash.as_deref().map(|h| verify_password(&req.password, h)).unwrap_or(false))
        .ok_or_else(|| {
            metrics::counter!("pixelhort_login_failures_total").increment(1);
            AppError::Unauthorized("incorrect email or password".into())
        })?;
    let token = create_session(&state, user.id, req.device_type, req.device_name).await?;
    let dto = user_dto(&state.db, &user).await?;
    Ok(login_response(token.clone(), LoginResponse { access_token: token, user: dto }))
}

async fn logout(State(state): State<SharedState>, headers: HeaderMap) -> AppResult<Response> {
    let (mut parts, _) = axum::http::Request::new(()).into_parts();
    parts.headers = headers;
    if let Some(token) = bearer_token(&parts) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1").bind(sha256_hex(&token)).execute(&state.db).await?;
    }
    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("ph_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    Ok(res)
}

async fn session_info(State(state): State<SharedState>, auth: Auth) -> AppResult<Json<Value>> {
    let unlocked_until: Option<chrono::DateTime<Utc>> = match auth.session_id {
        Some(id) => sqlx::query_scalar("SELECT unlocked_until FROM sessions WHERE id = $1")
            .bind(id)
            .fetch_one(&state.db)
            .await?,
        None => None,
    };
    Ok(Json(json!({
        "id": auth.session_id,
        "unlockedUntil": unlocked_until.filter(|t| *t > Utc::now()),
        "hasPin": auth.user.pin_hash.is_some(),
    })))
}

#[derive(Deserialize)]
struct PinRequest {
    pin: String,
}

async fn unlock(State(state): State<SharedState>, auth: Auth, Json(req): Json<PinRequest>) -> AppResult<Json<Value>> {
    let session = auth.session_id.ok_or_else(|| AppError::bad("locked folder requires a login session"))?;
    let hash = auth.user.pin_hash.as_deref().ok_or_else(|| AppError::bad("no PIN configured"))?;
    if !verify_password(&req.pin, hash) {
        return Err(AppError::Unauthorized("wrong PIN".into()));
    }
    let until = Utc::now() + Duration::minutes(15);
    sqlx::query("UPDATE sessions SET unlocked_until = $2 WHERE id = $1")
        .bind(session)
        .bind(until)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "unlockedUntil": until })))
}

async fn lock(State(state): State<SharedState>, auth: Auth) -> AppResult<StatusCode> {
    if let Some(session) = auth.session_id {
        sqlx::query("UPDATE sessions SET unlocked_until = NULL WHERE id = $1").bind(session).execute(&state.db).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- OAuth / OIDC

#[derive(Deserialize)]
struct Discovery {
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: Option<String>,
}

async fn discovery(state: &SharedState) -> AppResult<Discovery> {
    let cfg = state.sys().oauth;
    if !cfg.enabled || cfg.issuer_url.is_empty() {
        return Err(AppError::bad("OAuth is not enabled"));
    }
    let url = if cfg.issuer_url.contains("/.well-known/") {
        cfg.issuer_url.clone()
    } else {
        format!("{}/.well-known/openid-configuration", cfg.issuer_url.trim_end_matches('/'))
    };
    let d = state
        .http
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| AppError::Unavailable(format!("OAuth discovery failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Unavailable(format!("invalid discovery document: {e}")))?;
    Ok(d)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizeQuery {
    redirect_uri: String,
}

async fn oauth_authorize(State(state): State<SharedState>, Query(q): Query<AuthorizeQuery>) -> AppResult<Json<Value>> {
    let cfg = state.sys().oauth;
    let d = discovery(&state).await?;
    let state_param = random_token(24);
    let verifier = random_token(48);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    sqlx::query("INSERT INTO oauth_states (state, code_verifier, redirect_uri) VALUES ($1,$2,$3)")
        .bind(&state_param)
        .bind(&verifier)
        .bind(&q.redirect_uri)
        .execute(&state.db)
        .await?;
    let mut url = url::Url::parse(&d.authorization_endpoint).map_err(|e| AppError::Internal(e.into()))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &cfg.client_id)
        .append_pair("redirect_uri", &q.redirect_uri)
        .append_pair("scope", &cfg.scope)
        .append_pair("state", &state_param)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(Json(json!({ "url": url.to_string() })))
}

#[derive(Deserialize)]
struct CallbackRequest {
    url: String,
}

async fn oauth_callback(State(state): State<SharedState>, Json(req): Json<CallbackRequest>) -> AppResult<Response> {
    let cfg = state.sys().oauth;
    let parsed = url::Url::parse(&req.url)
        .or_else(|_| url::Url::parse(&format!("http://x/{}", req.url.trim_start_matches('/'))))
        .map_err(|_| AppError::bad("invalid callback url"))?;
    let param = |k: &str| parsed.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.to_string());
    if let Some(err) = param("error") {
        return Err(AppError::Unauthorized(format!("OAuth error: {err}")));
    }
    let code = param("code").ok_or_else(|| AppError::bad("code missing"))?;
    let st = param("state").ok_or_else(|| AppError::bad("state missing"))?;
    let row: Option<(String, String)> = sqlx::query_as(
        "DELETE FROM oauth_states WHERE state = $1 AND created_at > now() - interval '15 minutes'
         RETURNING code_verifier, redirect_uri",
    )
    .bind(&st)
    .fetch_optional(&state.db)
    .await?;
    let (verifier, redirect_uri) = row.ok_or_else(|| AppError::Unauthorized("invalid or expired OAuth state".into()))?;
    let d = discovery(&state).await?;
    let token: Value = state
        .http
        .post(&d.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| AppError::Unavailable(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::Unavailable(e.to_string()))?;
    let access = token["access_token"]
        .as_str()
        .ok_or_else(|| AppError::Unauthorized(format!("token exchange failed: {token}")))?;
    let userinfo_url = d.userinfo_endpoint.ok_or_else(|| AppError::Unavailable("provider has no userinfo endpoint".into()))?;
    let info: Value = state
        .http
        .get(userinfo_url)
        .bearer_auth(access)
        .send()
        .await
        .map_err(|e| AppError::Unavailable(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::Unavailable(e.to_string()))?;
    let sub = info["sub"].as_str().ok_or_else(|| AppError::Unauthorized("userinfo without sub".into()))?;
    let email = info["email"].as_str().map(|e| e.to_lowercase());
    let name = info["name"]
        .as_str()
        .or(info["preferred_username"].as_str())
        .or(email.as_deref())
        .unwrap_or("User")
        .to_string();

    let mut user: Option<UserRow> = sqlx::query_as("SELECT * FROM users WHERE oauth_id = $1")
        .bind(sub)
        .fetch_optional(&state.db)
        .await?;
    if user.is_none() {
        if let Some(email) = &email {
            user = sqlx::query_as("UPDATE users SET oauth_id = $2, updated_at = now() WHERE email = $1 RETURNING *")
                .bind(email)
                .bind(sub)
                .fetch_optional(&state.db)
                .await?;
        }
    }
    let user = match user {
        Some(u) => u,
        None if cfg.auto_register => {
            let email = email.ok_or_else(|| AppError::bad("OAuth provider did not return an email"))?;
            let first: bool = sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM users)").fetch_one(&state.db).await?;
            sqlx::query_as(
                "INSERT INTO users (id, email, name, oauth_id, is_admin) VALUES ($1,$2,$3,$4,$5) RETURNING *",
            )
            .bind(Uuid::new_v4())
            .bind(email)
            .bind(name)
            .bind(sub)
            .bind(first)
            .fetch_one(&state.db)
            .await?
        }
        None => return Err(AppError::Forbidden("user is not registered".into())),
    };
    let device = if redirect_uri.starts_with("http") { "web" } else { "mobile" };
    let token = create_session(&state, user.id, Some(device.into()), Some("OAuth".into())).await?;
    let dto = user_dto(&state.db, &user).await?;
    Ok(login_response(token.clone(), LoginResponse { access_token: token, user: dto }))
}
