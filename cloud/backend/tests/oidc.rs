mod common;

use std::time::Duration;

use bcrypt::DEFAULT_COST;
use chrono::{Duration as ChronoDuration, Utc};
use codex_cloud_backend::config::{JwksCacheSettings, OidcConfig};
use codex_cloud_backend::db::{self, ExternalIdentitySeed};
use codex_cloud_backend::error::AppError;
use codex_cloud_backend::models::TokenResponse;
use codex_cloud_backend::security::{OidcProvider, decode_token};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use common::TestApp;

const CLIENT_ID: &str = "test-client";
const CLIENT_SECRET: &str = "super-secret";
const SUBJECT: &str = "user-123";
const KEY_ID: &str = "test-key";
const PRIVATE_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCsYo1004GjSvYj\nl8bTcriVWuuPaYvjKMUBmuDuLeMEcT742Jx9/5hgiDQghI0TVUNgBC2WJSVTJVzY\nuQ51RTDNjKZvhJMxXWT2/qSWOgUz1SkIh+SKoPKzkoGncHhUEp8akoliJGQJNGw1\n9Hb6oUHFzgoK1/oo7ezJuCunpM5c7dPFJDVnOGs8K8Clvl5EevsDG1QRJoV7CavY\n+LtIZTCyoa3ddyaijHy3grgsxtNf3TwrX/q/DI9uSFqAJ21FGh9nSHFmIFtWlqsI\nWn2ibkUOrpbXS6WWREer7JylJYzWUKVuTk0Qxx3QBqu1VKwLVO6cM1Lgk9PFdMeo\nTKAgiObLAgMBAAECggEALNKJRm2yYROeMX4G8Db9oLQd2NHQUjXpF7Q+NSAQTbjm\nb0zfT/G0HLF9oFDm37aFSMN9WPN6o4ZtAFsJ09s0R9YA9rEplqXamVB32inm7WXJ\nABNZjOQxhxiahr97Qhz/aqjcePzOWAhd9J+Gij+Aux6KROyIerj2nzK4gyQallXA\nw4pdn3UgtoWdOjndd2o3l/qEZo58hK2Rq+g6rNZW2WhT0IfQ+C65GUsc7lVB6RPS\nqHmglP61vGdPN2uZuCMgR/Osldf2gEnkXq4u7Zvs9jtQAhVgtCZdbwVuvXwdfRsv\nZ1RmhgOuugsnrFa9/qH3BEn3y97jbpANKl1iKWto4QKBgQDnDaWFJ1i8M4O45PiL\n6p1M4v8S+XBcy9qdiho+f2mqtVas1ErtEgLc5XImViJkJ7Lh300R8ttmnIdJsmLO\ntGvm7qPl/lrYoDjNtGERM277+xmkm9VEfFl2GGd3DYBEsHHYcvx7NN+rKnKbuRzC\nOf/qhj9vnmtDY6rzrv51C+fDqwKBgQC+/02TYqnQ6oAOyURR+4aqqODeumuTcWpR\nBpZWjOuS2lqxOHiG34aboutU671FeQ3OMBjAobev70+xSKyaXNagUwaXgZKnBtqq\ngQcQFxHQt5Oe3vt6o9XOxMZu2npPEA3j6Bm/nWPhR9BaqEVedVqrfbl6FahOzLcP\nkkyXKMRJYQKBgBIAXC79m8o98TtMi5jLFKpS3TCrQnfYYhX4FodcAe2M503b1GKY\nDqULM1ONTmyjMyqp7SVC2JksBNZXEZ+tKuL5IMfgg09xXDuanB1s9m6nZ54NjhYh\n4g5zZExAPwga/yOwAb/PpMV/LyK2z2jKgAfTocmefBjqAP2vWp/f55S1AoGBAJ2j\nxBhwZ26KDbWmgqATtJtolWjffmiMRE6p3C2FU+26EP6SeFABb21Hc2p8w5PyjOVw\nJw3eq+gm4aSWMfeZxn8+54Lmq+71pkbyBa1PDSIyUkHfErqvPInTOWBHLInS21QO\nvim7srM+fYZFujNzMqm2M/7Cn06igjj07Aga2p8hAoGAVezqPQrZYoWOn8kpgjps\ndIkUwaS467y9wjIa7MwP5p7mwL/4zVzVzs6CvhPcE0A8se4WUchnrSndFRhOPpjb\n9JOmwpLFVYj/92aOSm4xxzEFnTkZBNzSEE9UEjery28Yn8r30i0ZYHyFWST+NfYL\nDC2ZXdQiakOHjSA5P9/QQdI=\n-----END PRIVATE KEY-----\n";
const JWK_MODULUS: &str = "rGKNdNOBo0r2I5fG03K4lVrrj2mL4yjFAZrg7i3jBHE--Nicff-YYIg0IISNE1VDYAQtliUlUyVc2LkOdUUwzYymb4STMV1k9v6kljoFM9UpCIfkiqDys5KBp3B4VBKfGpKJYiRkCTRsNfR2-qFBxc4KCtf6KO3sybgrp6TOXO3TxSQ1ZzhrPCvApb5eRHr7AxtUESaFewmr2Pi7SGUwsqGt3Xcmoox8t4K4LMbTX908K1_6vwyPbkhagCdtRRofZ0hxZiBbVparCFp9om5FDq6W10ullkRHq-ycpSWM1lClbk5NEMcd0AartVSsC1TunDNS4JPTxXTHqEygIIjmyw";
const JWK_EXPONENT: &str = "AQAB";

struct OidcFixture {
    _server: MockServer,
    issuer: String,
}

impl OidcFixture {
    async fn setup() -> Self {
        let mock = MockServer::start().await;
        let issuer = mock.uri();
        let token_endpoint = format!("{}/token", issuer);
        let jwks_uri = format!("{}/jwks", issuer);

        let encoding_key = EncodingKey::from_rsa_pem(PRIVATE_KEY_PEM.as_bytes()).expect("key");

        let expiration = (Utc::now() + ChronoDuration::minutes(5)).timestamp() as usize;
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(KEY_ID.to_string());
        let claims = json!({
            "sub": SUBJECT,
            "iss": issuer,
            "aud": CLIENT_ID,
            "exp": expiration,
            "email": "oidc@example.com",
            "name": "OIDC User"
        });
        let id_token = jsonwebtoken::encode(&header, &claims, &encoding_key).expect("token");

        Mock::given(method("GET"))
            .and(path("/.well-known/openid-configuration"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "issuer": issuer,
                "token_endpoint": token_endpoint,
                "jwks_uri": jwks_uri
            })))
            .mount(&mock)
            .await;

        Mock::given(method("GET"))
            .and(path("/jwks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "keys": [
                    {
                        "kty": "RSA",
                        "kid": KEY_ID,
                        "alg": "RS256",
                        "n": JWK_MODULUS,
                        "e": JWK_EXPONENT
                    }
                ]
            })))
            .mount(&mock)
            .await;

        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id_token": id_token,
                "access_token": "ignored"
            })))
            .mount(&mock)
            .await;

        Self {
            _server: mock,
            issuer,
        }
    }
}

#[tokio::test]
async fn oidc_callback_issues_token_for_linked_identity() {
    let fixture = OidcFixture::setup().await;
    let app = TestApp::spawn_with(|config| {
        config.oidc = Some(OidcConfig {
            issuer: fixture.issuer.clone(),
            client_id: CLIENT_ID.to_string(),
            client_secret: CLIENT_SECRET.to_string(),
            redirect_uri: "http://127.0.0.1:0/auth/oidc/callback".to_string(),
            jwks_cache: JwksCacheSettings {
                ttl: Duration::from_secs(3600),
                refresh: Duration::from_secs(60),
            },
        });
    })
    .await;

    let user_id = Uuid::new_v4();
    let created_at = Utc::now().to_rfc3339();
    let password_hash = bcrypt::hash("unused", DEFAULT_COST).expect("hash");
    sqlx::query(
        r#"
        INSERT INTO users (id, email, name, password_hash, auth_provider, created_at)
        VALUES (?, ?, ?, ?, 'oidc', ?)
        "#,
    )
    .bind(user_id.to_string())
    .bind("oidc@example.com")
    .bind("OIDC User")
    .bind(password_hash)
    .bind(&created_at)
    .execute(&app.pool)
    .await
    .expect("insert user");

    db::seed_external_identities(
        &app.pool,
        &[ExternalIdentitySeed {
            issuer: &fixture.issuer,
            subject: SUBJECT,
            user_id,
            email: Some("oidc@example.com"),
        }],
    )
    .await
    .expect("seed identity");

    let response = app
        .client
        .get(app.url("/auth/oidc/callback"))
        .query(&[("code", "test-code"), ("state", "ignored")])
        .send()
        .await
        .expect("response");
    assert!(response.status().is_success());

    let token = response.json::<TokenResponse>().await.expect("token");
    assert_eq!(token.token_type, "bearer");
    let subject = decode_token(&token.access_token, &app.config).expect("decode");
    assert_eq!(subject, user_id);
}

#[tokio::test]
async fn oidc_callback_rejects_unlinked_identity() {
    let fixture = OidcFixture::setup().await;
    let app = TestApp::spawn_with(|config| {
        config.oidc = Some(OidcConfig {
            issuer: fixture.issuer.clone(),
            client_id: CLIENT_ID.to_string(),
            client_secret: CLIENT_SECRET.to_string(),
            redirect_uri: "http://127.0.0.1:0/auth/oidc/callback".to_string(),
            jwks_cache: JwksCacheSettings {
                ttl: Duration::from_secs(3600),
                refresh: Duration::from_secs(60),
            },
        });
    })
    .await;

    let response = app
        .client
        .get(app.url("/auth/oidc/callback"))
        .query(&[("code", "test-code")])
        .send()
        .await
        .expect("response");

    assert_eq!(response.status(), http::StatusCode::UNAUTHORIZED);
    let body = response.json::<serde_json::Value>().await.expect("body");
    assert_eq!(body["detail"], "No account linked to external identity");
}

#[tokio::test]
async fn oidc_provider_discovery_rejects_mismatched_issuer() {
    let mock = MockServer::start().await;
    let config_issuer = mock.uri();
    let metadata_issuer = format!("{}/real", config_issuer);

    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": metadata_issuer,
            "token_endpoint": format!("{}/token", config_issuer),
            "jwks_uri": format!("{}/jwks", config_issuer)
        })))
        .mount(&mock)
        .await;

    let result = OidcProvider::discover(OidcConfig {
        issuer: config_issuer,
        client_id: CLIENT_ID.to_string(),
        client_secret: CLIENT_SECRET.to_string(),
        redirect_uri: "http://127.0.0.1:0/auth/oidc/callback".to_string(),
        jwks_cache: JwksCacheSettings {
            ttl: Duration::from_secs(3600),
            refresh: Duration::from_secs(60),
        },
    })
    .await;

    assert!(
        matches!(result, Err(AppError::BadRequest(message)) if message == "OIDC issuer mismatch")
    );
}
