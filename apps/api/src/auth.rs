use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::{PasswordHash, SaltString},
};
use axum::{
    Json, Router,
    extract::Request,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use tokio::sync::{Mutex, Semaphore};
use utoipa::ToSchema;

use crate::{ApiError, ErrorCode};

type AuthFailure = (StatusCode, Json<ApiError>);

#[derive(Clone)]
pub(super) struct PrivateState {
    pub pool: PgPool,
    config: AuthConfig,
}

pub(super) fn protect(
    routes: Router<PrivateState>,
    pool: PgPool,
    config: AuthConfig,
) -> Router<PgPool> {
    let state = PrivateState { pool, config };
    routes
        .route_layer(middleware::from_fn_with_state(state.clone(), private_guard))
        .with_state(state)
}

async fn private_guard(
    State(state): State<PrivateState>,
    request: Request,
    next: Next,
) -> Result<Response, AuthFailure> {
    let (_, _, stored_csrf_hash) =
        authenticated(&state.pool, &state.config, request.headers()).await?;
    if !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) {
        if !origin_valid(request.headers(), &state.config) {
            return Err(failure(
                StatusCode::FORBIDDEN,
                ErrorCode::Forbidden,
                "请求来源不匹配",
            ));
        }
        let supplied = request
            .headers()
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if hash(supplied.as_bytes())
            .ct_eq(&stored_csrf_hash)
            .unwrap_u8()
            != 1
        {
            return Err(failure(
                StatusCode::FORBIDDEN,
                ErrorCode::Forbidden,
                "CSRF 校验失败",
            ));
        }
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[derive(Clone)]
pub struct AuthConfig {
    origin: String,
    secure_cookie: bool,
    idle_seconds: i32,
    absolute_seconds: i32,
    hash_memory_kib: u32,
    hash_iterations: u32,
    hash_concurrency: usize,
    login_max_attempts: u32,
    login_window_seconds: u64,
}

impl AuthConfig {
    pub fn local() -> Self {
        Self::for_origin("http://127.0.0.1:5173").expect("固定开发地址有效")
    }

    pub fn for_origin(origin: &str) -> Result<Self, String> {
        let origin = origin.trim_end_matches('/');
        let uri: axum::http::Uri = origin.parse().map_err(|_| "AUTH_ORIGIN 格式无效")?;
        let scheme = uri.scheme_str().ok_or("AUTH_ORIGIN 缺少协议")?;
        let authority = uri.authority().ok_or("AUTH_ORIGIN 缺少主机")?;
        if uri.path() != "/" || uri.query().is_some() || authority.as_str().contains('@') {
            return Err("AUTH_ORIGIN 必须仅包含协议、主机和端口".into());
        }
        let secure_cookie = match scheme {
            "https" => true,
            "http" if matches!(authority.host(), "localhost" | "127.0.0.1" | "[::1]") => false,
            _ => return Err("非本机 AUTH_ORIGIN 必须使用 HTTPS".into()),
        };
        Ok(Self {
            origin: origin.to_string(),
            secure_cookie,
            idle_seconds: 1800,
            absolute_seconds: 604800,
            hash_memory_kib: 19456,
            hash_iterations: 2,
            hash_concurrency: 2,
            login_max_attempts: 5,
            login_window_seconds: 60,
        })
    }

    pub fn from_env() -> Result<Self, String> {
        let origin = std::env::var("AUTH_ORIGIN").map_err(|_| "缺少 AUTH_ORIGIN 环境变量")?;
        let mut config = Self::for_origin(&origin)?;
        config.idle_seconds = setting("AUTH_IDLE_SECONDS", 1800, 60, 604800)?;
        config.absolute_seconds = setting("AUTH_ABSOLUTE_SECONDS", 604800, 60, 2592000)?;
        if config.idle_seconds > config.absolute_seconds {
            return Err("AUTH_IDLE_SECONDS 不能大于 AUTH_ABSOLUTE_SECONDS".into());
        }
        config.hash_memory_kib = setting("AUTH_HASH_MEMORY_KIB", 19456, 19456, 262144)?;
        config.hash_iterations = setting("AUTH_HASH_ITERATIONS", 2, 2, 10)?;
        config.hash_concurrency = setting("AUTH_HASH_CONCURRENCY", 2, 1, 8)?;
        config.login_max_attempts = setting("AUTH_LOGIN_MAX_ATTEMPTS", 5, 1, 100)?;
        config.login_window_seconds = setting("AUTH_LOGIN_WINDOW_SECONDS", 60, 1, 3600)?;
        Ok(config)
    }

    fn cookie_name(&self) -> &'static str {
        if self.secure_cookie {
            "__Host-mf_session"
        } else {
            "mf_session"
        }
    }

    fn argon2(&self) -> Result<Argon2<'static>, String> {
        let params = Params::new(self.hash_memory_kib, self.hash_iterations, 1, None)
            .map_err(|_| "Argon2id 参数无效")?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

fn setting<T>(key: &str, default: T, min: T, max: T) -> Result<T, String>
where
    T: std::str::FromStr + Ord + Copy,
{
    let value = match std::env::var(key) {
        Ok(raw) => raw.parse().map_err(|_| format!("{key} 格式无效"))?,
        Err(std::env::VarError::NotPresent) => default,
        Err(_) => return Err(format!("{key} 不是有效文本")),
    };
    if value < min || value > max {
        return Err(format!("{key} 超出允许范围"));
    }
    Ok(value)
}

struct LoginWindow {
    started: Instant,
    attempts: u32,
}

#[derive(Clone)]
pub(super) struct AuthState {
    pool: PgPool,
    config: AuthConfig,
    hash_slots: Arc<Semaphore>,
    login_window: Arc<Mutex<LoginWindow>>,
}

pub(super) fn routes(pool: PgPool, config: AuthConfig) -> Router<PgPool> {
    let state = AuthState {
        pool,
        hash_slots: Arc::new(Semaphore::new(config.hash_concurrency)),
        login_window: Arc::new(Mutex::new(LoginWindow {
            started: Instant::now(),
            attempts: 0,
        })),
        config,
    };
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/session", get(session))
        .route("/auth/logout", post(logout))
        .with_state(state)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SessionResponse {
    username: String,
    csrf_token: String,
}

fn failure(status: StatusCode, code: ErrorCode, message: &str) -> AuthFailure {
    (
        status,
        Json(ApiError {
            code,
            message: message.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }),
    )
}

fn unauthorized() -> AuthFailure {
    failure(
        StatusCode::UNAUTHORIZED,
        ErrorCode::Unauthorized,
        "请先登录",
    )
}

fn origin_valid(headers: &HeaderMap, config: &AuthConfig) -> bool {
    headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        == Some(config.origin.as_str())
}

fn hash(value: &[u8]) -> Vec<u8> {
    Sha256::digest(value).to_vec()
}

fn csrf_token(session_token: &str) -> String {
    let mut input = b"mindfolio-csrf:".to_vec();
    input.extend_from_slice(session_token.as_bytes());
    URL_SAFE_NO_PAD.encode(Sha256::digest(input))
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn cookie_token(headers: &HeaderMap, config: &AuthConfig) -> Option<String> {
    let name = config.cookie_name();
    let value = headers.get(header::COOKIE)?.to_str().ok()?;
    let token = value.split(';').find_map(|part| {
        part.trim()
            .strip_prefix(name)
            .and_then(|part| part.strip_prefix('='))
    })?;
    if URL_SAFE_NO_PAD.decode(token).ok()?.len() != 32 {
        return None;
    }
    Some(token.into())
}

fn set_cookie(config: &AuthConfig, token: &str, max_age: i32) -> HeaderMap {
    let secure = if config.secure_cookie { "; Secure" } else { "" };
    let mut headers = HeaderMap::new();
    let value = format!(
        "{}={token}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Strict{secure}",
        config.cookie_name()
    );
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("Cookie 格式有效"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers
}

async fn check_login_limit(state: &AuthState) -> Result<(), AuthFailure> {
    let mut window = state.login_window.lock().await;
    if window.started.elapsed() >= Duration::from_secs(state.config.login_window_seconds) {
        window.started = Instant::now();
        window.attempts = 0;
    }
    if window.attempts >= state.config.login_max_attempts {
        return Err(failure(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            "登录尝试过于频繁",
        ));
    }
    window.attempts += 1;
    Ok(())
}

async fn password_hash(password: String, config: AuthConfig) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        config
            .argon2()?
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| "密码哈希失败".into())
    })
    .await
    .map_err(|_| "密码哈希任务失败".to_string())?
}

async fn verify_password(
    password: String,
    stored_hash: Option<String>,
    state: &AuthState,
) -> Result<bool, AuthFailure> {
    let permit = state
        .hash_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| {
            failure(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::AuthUnavailable,
                "认证服务暂不可用",
            )
        })?;
    let config = state.config.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let hasher = config.argon2().ok()?;
        match stored_hash {
            Some(stored) => {
                let parsed = PasswordHash::new(&stored).ok()?;
                Some(hasher.verify_password(password.as_bytes(), &parsed).is_ok())
            }
            None => {
                let salt = SaltString::encode_b64(b"mindfolio-dummy-salt").ok()?;
                let _ = hasher.hash_password(password.as_bytes(), &salt);
                Some(false)
            }
        }
    })
    .await
    .map_err(|_| {
        failure(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::AuthUnavailable,
            "认证服务暂不可用",
        )
    })?;
    Ok(result.unwrap_or(false))
}

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    description = "浏览器 Origin 必须与 AUTH_ORIGIN 一致",
    responses(
        (status = 200, description = "登录成功", body = SessionResponse, headers(("set-cookie" = String, description = "HttpOnly 管理端会话"))),
        (status = 400, description = "请求无效", body = ApiError),
        (status = 401, description = "账号或密码错误", body = ApiError),
        (status = 403, description = "请求来源不匹配", body = ApiError),
        (status = 429, description = "登录尝试过于频繁", body = ApiError),
        (status = 503, description = "认证服务暂不可用", body = ApiError)
    )
)]
pub(super) async fn login(
    State(state): State<AuthState>,
    headers: HeaderMap,
    input: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AuthFailure> {
    if !origin_valid(&headers, &state.config) {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "请求来源不匹配",
        ));
    }
    let Json(input) = input.map_err(|_| {
        failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            "请求无效",
        )
    })?;
    if input.username.trim().is_empty()
        || input.username.len() > 100
        || input.password.len() < 12
        || input.password.len() > 1024
    {
        return Err(failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            "请求无效",
        ));
    }
    check_login_limit(&state).await?;
    let account: Option<(i64, String)> =
        sqlx::query_as("SELECT id, password_hash FROM admin_account WHERE username = $1")
            .bind(&input.username)
            .fetch_optional(&state.pool)
            .await
            .map_err(|_| {
                failure(
                    StatusCode::SERVICE_UNAVAILABLE,
                    ErrorCode::AuthUnavailable,
                    "认证服务暂不可用",
                )
            })?;
    if !verify_password(
        input.password,
        account.as_ref().map(|(_, hash)| hash.clone()),
        &state,
    )
    .await?
    {
        return Err(failure(
            StatusCode::UNAUTHORIZED,
            ErrorCode::InvalidCredentials,
            "账号或密码错误",
        ));
    }
    let admin_id = account.expect("密码验证成功意味着账号存在").0;
    if let Some(old_token) = cookie_token(&headers, &state.config) {
        sqlx::query("UPDATE admin_session SET revoked_at = now() WHERE token_hash = $1 AND revoked_at IS NULL")
            .bind(hash(old_token.as_bytes()))
            .execute(&state.pool)
            .await
            .map_err(|_| failure(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::AuthUnavailable, "认证服务暂不可用"))?;
    }
    let token = random_token();
    let csrf = csrf_token(&token);
    sqlx::query(
        "INSERT INTO admin_session (admin_id, token_hash, csrf_token_hash, idle_expires_at, absolute_expires_at)
         VALUES ($1, $2, $3, now() + make_interval(secs => $4), now() + make_interval(secs => $5))",
    )
    .bind(admin_id)
    .bind(hash(token.as_bytes()))
    .bind(hash(csrf.as_bytes()))
    .bind(state.config.idle_seconds)
    .bind(state.config.absolute_seconds)
    .execute(&state.pool)
    .await
    .map_err(|_| failure(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::AuthUnavailable, "认证服务暂不可用"))?;
    Ok((
        set_cookie(&state.config, &token, state.config.absolute_seconds),
        Json(SessionResponse {
            username: input.username,
            csrf_token: csrf,
        }),
    ))
}

async fn authenticated(
    pool: &PgPool,
    config: &AuthConfig,
    headers: &HeaderMap,
) -> Result<(String, String, Vec<u8>), AuthFailure> {
    let token = cookie_token(headers, config).ok_or_else(unauthorized)?;
    let found: Option<(String, Vec<u8>)> = sqlx::query_as(
        "UPDATE admin_session AS s
         SET last_seen_at = now(),
             idle_expires_at = LEAST(now() + make_interval(secs => $2), absolute_expires_at)
         FROM admin_account AS a
         WHERE s.admin_id = a.id AND s.token_hash = $1 AND s.revoked_at IS NULL
           AND s.idle_expires_at > now() AND s.absolute_expires_at > now()
         RETURNING a.username, s.csrf_token_hash",
    )
    .bind(hash(token.as_bytes()))
    .bind(config.idle_seconds)
    .fetch_optional(pool)
    .await
    .map_err(|_| {
        failure(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::AuthUnavailable,
            "认证服务暂不可用",
        )
    })?;
    let (username, stored_csrf_hash) = found.ok_or_else(unauthorized)?;
    Ok((username, token, stored_csrf_hash))
}

#[utoipa::path(
    get,
    path = "/auth/session",
    security(("admin_session" = [])),
    responses(
        (status = 200, description = "当前会话", body = SessionResponse),
        (status = 401, description = "未登录或会话过期", body = ApiError),
        (status = 503, description = "认证服务暂不可用", body = ApiError)
    )
)]
pub(super) async fn session(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthFailure> {
    let (username, token, _) = authenticated(&state.pool, &state.config, &headers).await?;
    let mut response_headers = HeaderMap::new();
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((
        response_headers,
        Json(SessionResponse {
            username,
            csrf_token: csrf_token(&token),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    security(("admin_session" = [])),
    description = "浏览器 Origin 必须与 AUTH_ORIGIN 一致",
    params(("x-csrf-token" = String, Header, description = "当前会话的 CSRF token")),
    responses(
        (status = 204, description = "退出成功", headers(("set-cookie" = String, description = "清除会话 Cookie"))),
        (status = 401, description = "未登录或会话过期", body = ApiError),
        (status = 403, description = "CSRF 或来源校验失败", body = ApiError),
        (status = 503, description = "认证服务暂不可用", body = ApiError)
    )
)]
pub(super) async fn logout(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AuthFailure> {
    if !origin_valid(&headers, &state.config) {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "请求来源不匹配",
        ));
    }
    let (_, token, stored_csrf_hash) = authenticated(&state.pool, &state.config, &headers).await?;
    let supplied = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if hash(supplied.as_bytes())
        .ct_eq(&stored_csrf_hash)
        .unwrap_u8()
        != 1
    {
        return Err(failure(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "CSRF 校验失败",
        ));
    }
    sqlx::query("UPDATE admin_session SET revoked_at = now() WHERE token_hash = $1")
        .bind(hash(token.as_bytes()))
        .execute(&state.pool)
        .await
        .map_err(|_| {
            failure(
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorCode::AuthUnavailable,
                "认证服务暂不可用",
            )
        })?;
    Ok((set_cookie(&state.config, "", 0), StatusCode::NO_CONTENT))
}

pub async fn initialize_admin(
    pool: &PgPool,
    username: &str,
    password: String,
    config: AuthConfig,
) -> Result<(), String> {
    if username.trim() != username || username.is_empty() || username.len() > 100 {
        return Err("管理者账号格式无效".into());
    }
    if !(12..=1024).contains(&password.len()) {
        return Err("密码长度必须为 12 至 1024 字节".into());
    }
    let password_hash = password_hash(password, config).await?;
    let mut tx = pool.begin().await.map_err(|_| "数据库事务启动失败")?;
    sqlx::query("SELECT pg_advisory_xact_lock(771101)")
        .execute(&mut *tx)
        .await
        .map_err(|_| "管理者初始化锁定失败")?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admin_account)")
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| "管理者账号查询失败")?;
    if exists {
        return Err("管理者已初始化，拒绝重复初始化".into());
    }
    sqlx::query("INSERT INTO admin_account (username, password_hash) VALUES ($1, $2)")
        .bind(username)
        .bind(password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|_| "管理者账号写入失败")?;
    tx.commit().await.map_err(|_| "管理者初始化提交失败")?;
    Ok(())
}

pub async fn reset_admin(
    pool: &PgPool,
    password: String,
    config: AuthConfig,
) -> Result<(), String> {
    if !(12..=1024).contains(&password.len()) {
        return Err("密码长度必须为 12 至 1024 字节".into());
    }
    let password_hash = password_hash(password, config).await?;
    let mut tx = pool.begin().await.map_err(|_| "数据库事务启动失败")?;
    let updated =
        sqlx::query("UPDATE admin_account SET password_hash = $1, updated_at = now() WHERE id = 1")
            .bind(password_hash)
            .execute(&mut *tx)
            .await
            .map_err(|_| "管理者密码更新失败")?;
    if updated.rows_affected() != 1 {
        return Err("管理者尚未初始化".into());
    }
    sqlx::query(
        "UPDATE admin_session SET revoked_at = now() WHERE admin_id = 1 AND revoked_at IS NULL",
    )
    .execute(&mut *tx)
    .await
    .map_err(|_| "旧会话撤销失败")?;
    tx.commit().await.map_err(|_| "管理者密码重置提交失败")?;
    Ok(())
}
