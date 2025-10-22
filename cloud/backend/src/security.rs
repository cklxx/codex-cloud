use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{FromRef, FromRequestParts};
use axum::http::{header::AUTHORIZATION, request::Parts};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::future::Future;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::models::User;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
struct TokenClaims {
    sub: Uuid,
    exp: usize,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    Ok(bcrypt::hash(password, bcrypt::DEFAULT_COST)?)
}

pub fn verify_password(password: &str, hashed: &str) -> bool {
    bcrypt::verify(password, hashed).unwrap_or(false)
}

pub fn create_access_token(user_id: Uuid, config: &AppConfig) -> Result<String, AppError> {
    let expiration = SystemTime::now()
        .checked_add(Duration::from_secs(config.access_token_expire_minutes * 60))
        .unwrap_or(SystemTime::now());
    let exp = expiration
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as usize;
    let claims = TokenClaims { sub: user_id, exp };
    let token = jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.secret_key.as_bytes()),
    )?;
    Ok(token)
}

pub fn decode_token(token: &str, config: &AppConfig) -> Result<Uuid, AppError> {
    let validation = Validation::default();
    let data = jsonwebtoken::decode::<TokenClaims>(
        token,
        &DecodingKey::from_secret(config.secret_key.as_bytes()),
        &validation,
    )?;
    Ok(data.claims.sub)
}

pub async fn fetch_user(pool: &SqlitePool, user_id: Uuid) -> Result<User, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id, email, name
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or_else(|| AppError::not_found("User not found"))?;
    let id: String = row.try_get("id")?;
    let email: String = row.try_get("email")?;
    let name: Option<String> = row.try_get("name")?;
    Ok(User {
        id: Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Invalid user id"))?,
        email,
        name,
    })
}

pub struct CurrentUser(pub User);

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts<'a>(
        parts: &'a mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let state = AppState::from_ref(state);
        async move {
            let header = parts
                .headers
                .get(AUTHORIZATION)
                .ok_or_else(|| AppError::unauthorized("Missing Authorization header"))?;
            let text = header
                .to_str()
                .map_err(|_| AppError::unauthorized("Invalid Authorization header"))?;
            let token = text
                .strip_prefix("Bearer ")
                .ok_or_else(|| AppError::unauthorized("Invalid Authorization header"))?;

            let user_id = decode_token(token, &state.config)?;
            let user = fetch_user(&state.pool, user_id).await?;
            Ok(Self(user))
        }
    }
}
