mod common;

use common::TestApp;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn end_to_end_task_flow() {
    let app = TestApp::spawn().await;

    let register = app
        .client
        .post(app.url("/auth/users"))
        .json(&json!({
            "email": "admin@example.com",
            "password": "secret123",
            "name": "Admin"
        }))
        .send()
        .await
        .unwrap();
    assert!(
        register.status().is_success(),
        "register failed: {:?}",
        register.text().await.ok()
    );

    let login = app
        .client
        .post(app.url("/auth/session"))
        .json(&json!({
            "email": "admin@example.com",
            "password": "secret123"
        }))
        .send()
        .await
        .unwrap();
    assert!(login.status().is_success());
    let token = login.json::<serde_json::Value>().await.unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let auth_header = format!("Bearer {token}");

    let repo = app
        .client
        .post(app.url("/repositories"))
        .header("Authorization", &auth_header)
        .json(&json!({
            "name": "codex",
            "git_url": "https://example.com/codex.git",
            "default_branch": "main"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(repo.status(), 201);
    let repo_body = repo.json::<serde_json::Value>().await.unwrap();
    let repository_id = Uuid::parse_str(repo_body["id"].as_str().unwrap()).unwrap();

    let task = app
        .client
        .post(app.url("/tasks"))
        .header("Authorization", &auth_header)
        .json(&json!({
            "title": "Implement MVP",
            "description": "Build minimal Codex Cloud backend",
            "repository_id": repository_id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(task.status(), 201);
    let task_body = task.json::<serde_json::Value>().await.unwrap();
    let task_id = Uuid::parse_str(task_body["id"].as_str().unwrap()).unwrap();

    let claim = app
        .client
        .post(app.url(&format!("/tasks/{task_id}/claim")))
        .header("Authorization", &auth_header)
        .send()
        .await
        .unwrap();
    assert!(claim.status().is_success());

    let attempt = app
        .client
        .post(app.url(&format!("/tasks/{task_id}/attempts")))
        .header("Authorization", &auth_header)
        .json(&json!({"environment_id": "default"}))
        .send()
        .await
        .unwrap();
    assert_eq!(attempt.status(), 201);
    let attempt_body = attempt.json::<serde_json::Value>().await.unwrap();
    let attempt_id = Uuid::parse_str(attempt_body["id"].as_str().unwrap()).unwrap();

    let complete = app
        .client
        .post(app.url(&format!("/tasks/attempts/{attempt_id}/complete")))
        .header("Authorization", &auth_header)
        .json(&json!({
            "status": "succeeded",
            "diff": "diff --git a/file b/file",
            "log": "execution log"
        }))
        .send()
        .await
        .unwrap();
    assert!(complete.status().is_success());
    let complete_body = complete.json::<serde_json::Value>().await.unwrap();
    assert_eq!(complete_body["status"], "succeeded");
    let diff_url = complete_body["diff_url"].as_str().unwrap().to_string();

    let detail = app
        .client
        .get(app.url(&format!("/tasks/{task_id}")))
        .header("Authorization", &auth_header)
        .send()
        .await
        .unwrap();
    assert!(detail.status().is_success());
    let detail_body = detail.json::<serde_json::Value>().await.unwrap();
    assert_eq!(detail_body["status"], "review");
    assert_eq!(detail_body["attempts"].as_array().unwrap().len(), 1);

    let artifact = app.client.get(diff_url).send().await.unwrap();
    assert!(artifact.status().is_success());
    let artifact_body = artifact.text().await.unwrap();
    assert!(artifact_body.contains("diff --git"));

    let tasks = app
        .client
        .get(app.url("/tasks"))
        .header("Authorization", &auth_header)
        .send()
        .await
        .unwrap();
    assert!(tasks.status().is_success());
    let tasks_body = tasks.json::<serde_json::Value>().await.unwrap();
    assert!(
        tasks_body
            .as_array()
            .unwrap()
            .iter()
            .any(|task| task["id"] == detail_body["id"])
    );
}
