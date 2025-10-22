mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn creating_task_without_auth_returns_unauthorized() {
    let app = TestApp::spawn().await;

    let response = app
        .client
        .post(app.url("/tasks"))
        .json(&json!({
            "title": "Test",
            "description": "Unauthorized",
            "repository_id": Uuid::new_v4(),
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response.json::<serde_json::Value>().await.unwrap();
    assert_eq!(body["detail"], "Missing Authorization header");
}
