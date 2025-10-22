mod common;

use common::TestApp;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn codex_compatibility_endpoints_create_task() {
    let app = TestApp::spawn().await;

    // Register user
    let _ = app
        .client
        .post(app.url("/auth/users"))
        .json(&json!({
            "email": "cli@example.com",
            "password": "secret123",
            "name": "CLI User"
        }))
        .send()
        .await
        .unwrap();

    let login = app
        .client
        .post(app.url("/auth/session"))
        .json(&json!({
            "email": "cli@example.com",
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
            "git_url": "https://github.com/example/codex.git",
            "default_branch": "main"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(repo.status(), 201);
    let repo_body = repo.json::<serde_json::Value>().await.unwrap();
    let repository_id = Uuid::parse_str(repo_body["id"].as_str().unwrap()).unwrap();

    let env = app
        .client
        .post(app.url("/environments"))
        .header("Authorization", &auth_header)
        .json(&json!({
            "id": "local-dev",
            "label": "Local Dev",
            "repository_id": repository_id,
            "branch": "main",
            "is_pinned": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(env.status(), 201);

    let environments = app
        .client
        .get(app.url("/api/codex/environments"))
        .header("Authorization", &auth_header)
        .send()
        .await
        .unwrap();
    assert!(environments.status().is_success());
    let env_list = environments.json::<serde_json::Value>().await.unwrap();
    assert!(
        env_list
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "local-dev")
    );

    let by_repo = app
        .client
        .get(app.url("/api/codex/environments/by-repo/github/example/codex"))
        .header("Authorization", &auth_header)
        .send()
        .await
        .unwrap();
    assert!(by_repo.status().is_success());
    let by_repo_body = by_repo.json::<serde_json::Value>().await.unwrap();
    assert_eq!(by_repo_body.as_array().unwrap().len(), 1);

    let codex_task = app
        .client
        .post(app.url("/api/codex/tasks"))
        .header("Authorization", &auth_header)
        .json(&json!({
            "new_task": {
                "environment_id": "local-dev",
                "branch": "main",
                "run_environment_in_qa_mode": false
            },
            "input_items": [
                {
                    "type": "message",
                    "role": "user",
                    "content": [
                        { "content_type": "text", "text": "Implement CLI compatibility" }
                    ]
                }
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(codex_task.status(), 201);
    let codex_body = codex_task.json::<serde_json::Value>().await.unwrap();
    let task_id =
        Uuid::parse_str(codex_body["task"]["id"].as_str().expect("task id present")).unwrap();
    assert_eq!(codex_body["task"]["environment_id"], "local-dev");

    let detail = app
        .client
        .get(app.url(&format!("/tasks/{task_id}")))
        .header("Authorization", &auth_header)
        .send()
        .await
        .unwrap();
    assert!(detail.status().is_success());
    let detail_body = detail.json::<serde_json::Value>().await.unwrap();
    assert_eq!(detail_body["environment_id"], "local-dev");
    assert_eq!(detail_body["title"], "Implement CLI compatibility");
}
