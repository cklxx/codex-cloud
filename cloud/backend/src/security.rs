use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{FromRef, FromRequestParts};
use axum::http::{header::AUTHORIZATION, request::Parts};
use jsonwebtoken::{self, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::future::Future;
use uuid::Uuid;

use tokio::sync::RwLock;

use crate::config::{AppConfig, JwksCacheSettings, OidcConfig};
use crate::error::AppError;
use crate::models::User;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct OidcClaims {
    pub subject: String,
    pub email: Option<String>,
    pub name: Option<String>,
}

#[derive(Clone)]
pub struct OidcProvider {
    client: reqwest::Client,
    issuer: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    metadata: Arc<OidcMetadata>,
    jwks_cache: Arc<RwLock<Option<CachedJwks>>>,
    cache_settings: JwksCacheSettings,
}

#[derive(Clone, Debug)]
struct OidcMetadata {
    token_endpoint: Url,
    jwks_uri: Url,
}

#[derive(Clone, Debug)]
struct CachedJwks {
    keys: JsonWebKeySet,
    fetched_at: Instant,
}

#[derive(Debug, Deserialize)]
struct ProviderMetadata {
    issuer: String,
    token_endpoint: String,
    jwks_uri: String,
}

#[derive(Clone, Debug, Deserialize)]
struct JsonWebKeySet {
    keys: Vec<JsonWebKey>,
}

#[derive(Clone, Debug, Deserialize)]
struct JsonWebKey {
    kid: Option<String>,
    kty: String,
    #[allow(dead_code)]
    alg: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    sub: String,
    iss: String,
    #[serde(deserialize_with = "deserialize_audience")]
    aud: Vec<String>,
    email: Option<String>,
    name: Option<String>,
    #[allow(dead_code)]
    exp: usize,
}

#[derive(Debug, Deserialize)]
struct TokenEndpointResponse {
    id_token: String,
}

#[derive(Serialize)]
struct TokenEndpointRequest<'a> {
    grant_type: &'static str,
    code: &'a str,
    redirect_uri: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
}

impl OidcProvider {
    pub async fn discover(config: OidcConfig) -> Result<Self, AppError> {
        let client = reqwest::Client::builder().build()?;
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            config.issuer.trim_end_matches('/')
        );
        let metadata = client
            .get(&discovery_url)
            .send()
            .await?
            .error_for_status()?
            .json::<ProviderMetadata>()
            .await?;

        if metadata.issuer != config.issuer {
            return Err(AppError::bad_request("OIDC issuer mismatch"));
        }

        let token_endpoint = Url::parse(&metadata.token_endpoint)
            .map_err(|_| AppError::bad_request("Invalid token endpoint"))?;
        let jwks_uri = Url::parse(&metadata.jwks_uri)
            .map_err(|_| AppError::bad_request("Invalid JWKS URI"))?;

        Ok(Self {
            client,
            issuer: config.issuer,
            client_id: config.client_id,
            client_secret: config.client_secret,
            redirect_uri: config.redirect_uri,
            metadata: Arc::new(OidcMetadata {
                token_endpoint,
                jwks_uri,
            }),
            jwks_cache: Arc::new(RwLock::new(None)),
            cache_settings: config.jwks_cache,
        })
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub async fn exchange_code(&self, code: &str) -> Result<String, AppError> {
        let body = TokenEndpointRequest {
            grant_type: "authorization_code",
            code,
            redirect_uri: &self.redirect_uri,
            client_id: &self.client_id,
            client_secret: &self.client_secret,
        };

        let response = self
            .client
            .post(self.metadata.token_endpoint.clone())
            .form(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<TokenEndpointResponse>()
            .await?;

        if response.id_token.is_empty() {
            return Err(AppError::unauthorized("Missing id_token in response"));
        }

        Ok(response.id_token)
    }

    pub async fn validate_id_token(&self, token: &str) -> Result<OidcClaims, AppError> {
        let header = jsonwebtoken::decode_header(token)?;
        let alg = header.alg;
        if !matches!(alg, Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512) {
            return Err(AppError::unauthorized("Unsupported signing algorithm"));
        }
        let kid = header
            .kid
            .ok_or_else(|| AppError::unauthorized("Missing key id in token header"))?;

        let jwks = self.load_jwks(&kid).await?;
        let jwk = jwks
            .keys
            .iter()
            .find(|key| key.kid.as_deref() == Some(&kid))
            .ok_or_else(|| AppError::unauthorized("Signing key not found"))?;
        let decoding_key = jwk.decoding_key()?;

        let mut validation = Validation::new(alg);
        validation.validate_aud = false;
        validation.set_issuer(&[self.issuer.clone()]);
        validation.leeway = 5;

        let claims =
            jsonwebtoken::decode::<IdTokenClaims>(token, &decoding_key, &validation)?.claims;

        if !claims.aud.iter().any(|aud| aud == &self.client_id) {
            return Err(AppError::unauthorized("Invalid audience"));
        }

        if claims.iss != self.issuer {
            return Err(AppError::unauthorized("Invalid issuer"));
        }

        Ok(OidcClaims {
            subject: claims.sub,
            email: claims.email,
            name: claims.name,
        })
    }

    async fn load_jwks(&self, kid: &str) -> Result<JsonWebKeySet, AppError> {
        let now = Instant::now();
        let should_refresh = {
            let cache = self.jwks_cache.read().await;
            if let Some(cached) = &*cache {
                let age = now.duration_since(cached.fetched_at);
                let has_key = cached
                    .keys
                    .keys
                    .iter()
                    .any(|key| key.kid.as_deref() == Some(kid));

                if has_key && age <= self.cache_settings.refresh {
                    return Ok(cached.keys.clone());
                }

                age > self.cache_settings.ttl || !has_key || age > self.cache_settings.refresh
            } else {
                true
            }
        };

        if should_refresh {
            return self.refresh_jwks().await;
        }

        let cache = self.jwks_cache.read().await;
        if let Some(cached) = &*cache {
            return Ok(cached.keys.clone());
        }

        self.refresh_jwks().await
    }

    async fn refresh_jwks(&self) -> Result<JsonWebKeySet, AppError> {
        let keys = self
            .client
            .get(self.metadata.jwks_uri.clone())
            .send()
            .await?
            .error_for_status()?
            .json::<JsonWebKeySet>()
            .await?;

        let mut cache = self.jwks_cache.write().await;
        *cache = Some(CachedJwks {
            keys: keys.clone(),
            fetched_at: Instant::now(),
        });
        Ok(keys)
    }
}

impl JsonWebKey {
    fn decoding_key(&self) -> Result<DecodingKey, AppError> {
        if self.kty != "RSA" {
            return Err(AppError::unauthorized("Unsupported key type"));
        }

        let n = self
            .n
            .as_deref()
            .ok_or_else(|| AppError::unauthorized("Missing modulus on JWKS"))?;
        let e = self
            .e
            .as_deref()
            .ok_or_else(|| AppError::unauthorized("Missing exponent on JWKS"))?;

        Ok(DecodingKey::from_rsa_components(n, e)?)
    }
}

fn deserialize_audience<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, SeqAccess, Visitor};
    use std::fmt;

    struct AudienceVisitor;

    impl<'de> Visitor<'de> for AudienceVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element::<String>()? {
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_any(AudienceVisitor)
}

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
