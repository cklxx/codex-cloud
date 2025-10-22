mod common;

use common::TestApp;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn missing_artifact_returns_not_found() {
    let app = TestApp::spawn().await;

    let response = app
        .client
        .get(app.url("/artifacts/nonexistent.diff"))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.json::<Value>().await.unwrap();
    assert_eq!(body["detail"], "Artifact not found");
}
