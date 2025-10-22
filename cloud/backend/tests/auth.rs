mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn login_rejects_invalid_credentials() {
    let app = TestApp::spawn().await;

    let response = app
        .client
        .post(app.url("/auth/users"))
        .json(&json!({
            "email": "user@example.com",
            "password": "correct-horse",
            "name": "Tester"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let bad_login = app
        .client
        .post(app.url("/auth/session"))
        .json(&json!({
            "email": "user@example.com",
            "password": "tr0ub4dor"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_login.status(), StatusCode::UNAUTHORIZED);
    let body = bad_login.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["detail"], "Invalid credentials");
}

#[tokio::test]
async fn duplicate_registration_returns_conflict() {
    let app = TestApp::spawn().await;
    let payload = json!({
        "email": "dupe@example.com",
        "password": "secret",
        "name": "User"
    });

    let first = app
        .client
        .post(app.url("/auth/users"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    let duplicate = app
        .client
        .post(app.url("/auth/users"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let body = duplicate.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["detail"], "User already exists");
}
