use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Error as AnyError, Result, anyhow};
use chrono::Utc;
use clap::Parser;
use futures::stream::{self, StreamExt};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(author, version, about = "Codex Cloud task supervisor", long_about = None)]
struct Args {
    /// Base URL for the Codex Cloud API (e.g. http://127.0.0.1:8000)
    #[arg(
        long,
        env = "CODEX_CLOUD_API_BASE",
        default_value = "http://127.0.0.1:8000"
    )]
    api_base: String,

    /// Email for the service account used to authenticate with the API
    #[arg(
        long,
        env = "CODEX_CLOUD_EMAIL",
        default_value = "codex-cli@example.com"
    )]
    email: String,

    /// Password for the service account used to authenticate with the API
    #[arg(long, env = "CODEX_CLOUD_PASSWORD", default_value = "codex-cli")]
    password: String,

    /// Polling interval in seconds when waiting for new tasks
    #[arg(long, env = "CODEX_CLOUD_POLL_INTERVAL", default_value_t = 5)]
    poll_interval: u64,

    /// Optional environment filter. When set, only tasks tied to this environment are executed.
    #[arg(long, env = "CODEX_CLOUD_ENVIRONMENT_ID")]
    environment_id: Option<String>,

    /// Maximum number of attempts to execute concurrently
    #[arg(long, env = "CODEX_CLOUD_MAX_CONCURRENCY", default_value_t = 1)]
    max_concurrency: usize,
}

#[derive(Debug, Clone)]
struct Config {
    api_base: String,
    email: String,
    password: String,
    poll_interval: Duration,
    environment_id: Option<String>,
    max_concurrency: usize,
}

impl From<Args> for Config {
    fn from(args: Args) -> Self {
        Self {
            api_base: args.api_base.trim_end_matches('/').to_string(),
            email: args.email,
            password: args.password,
            poll_interval: Duration::from_secs(args.poll_interval.max(1)),
            environment_id: args.environment_id,
            max_concurrency: args.max_concurrency.max(1),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum TaskStatus {
    Pending,
    Claimed,
    Running,
    Review,
    Applied,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum AttemptStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Deserialize)]
struct TaskListResponse {
    id: Uuid,
    title: String,
    #[serde(default)]
    environment_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TaskDetailResponse {
    id: Uuid,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    environment_id: Option<String>,
    #[serde(default)]
    repository: Option<RepositorySummary>,
}

#[derive(Debug, Clone, Deserialize)]
struct RepositorySummary {
    id: Uuid,
    name: String,
    git_url: String,
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct AttemptRead {
    id: Uuid,
}

#[derive(Debug, Serialize)]
struct AttemptCreateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    environment_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct AttemptCompleteRequest {
    status: AttemptStatus,
    diff: Option<String>,
    log: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

struct AttemptContext {
    task: TaskListResponse,
    attempt: AttemptRead,
    detail: Option<TaskDetailResponse>,
}

struct AttemptArtifacts {
    diff: Option<String>,
    log: Option<String>,
}

struct SupervisorInner {
    client: Client,
    config: Config,
    token: RwLock<String>,
}

#[derive(Clone)]
struct Supervisor {
    inner: Arc<SupervisorInner>,
}

impl Supervisor {
    async fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .user_agent("codex-cloud-supervisor/0.1.0")
            .build()?;
        let token = Self::authenticate(&client, &config).await?;

        info!("Initial access token acquired");

        Ok(Self {
            inner: Arc::new(SupervisorInner {
                client,
                config,
                token: RwLock::new(token),
            }),
        })
    }

    async fn run(self) -> Result<()> {
        info!(
            max_concurrency = self.config().max_concurrency,
            "Supervisor started"
        );
        loop {
            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("Shutdown signal received");
                    break;
                }
                result = self.process_cycle() => {
                    if let Err(err) = result {
                        warn!(error = %err, "Supervisor cycle failed");
                    }
                }
            }
        }
        Ok(())
    }

    fn client(&self) -> &Client {
        &self.inner.client
    }

    fn config(&self) -> &Config {
        &self.inner.config
    }

    async fn process_cycle(&self) -> Result<()> {
        self.process_pending_tasks().await?;
        sleep(self.config().poll_interval).await;
        Ok(())
    }

    async fn process_pending_tasks(&self) -> Result<()> {
        let tasks = self.list_tasks(TaskStatus::Pending).await?;
        if tasks.is_empty() {
            info!("No pending tasks found");
            return Ok(());
        }

        let to_execute: Vec<_> = tasks
            .into_iter()
            .filter(|task| self.should_execute(task))
            .collect();

        if to_execute.is_empty() {
            info!("No pending tasks matched configured filters");
            return Ok(());
        }

        let max_concurrency = self.config().max_concurrency;
        stream::iter(to_execute.into_iter().map(|task| {
            let supervisor = self.clone();
            async move {
                let task_id = task.id;
                let title = task.title.clone();
                match supervisor.execute_task(task).await {
                    Ok(()) => {
                        info!(task_id = %task_id, title = %title, "Task completed");
                    }
                    Err(err) => {
                        warn!(
                            task_id = %task_id,
                            title = %title,
                            error = %err,
                            "Failed to execute task"
                        );
                    }
                }
            }
        }))
        .buffer_unordered(max_concurrency)
        .for_each(|_| async {})
        .await;

        Ok(())
    }

    fn should_execute(&self, task: &TaskListResponse) -> bool {
        match &self.config().environment_id {
            Some(filter) => task.environment_id.as_deref() == Some(filter.as_str()),
            None => true,
        }
    }

    async fn execute_task(&self, task: TaskListResponse) -> Result<()> {
        info!(task_id = %task.id, title = %task.title, "Attempting to claim task");
        let Some(context) = self.start_attempt(task).await? else {
            return Ok(());
        };

        match self.run_attempt(&context).await {
            Ok(artifacts) => {
                self.complete_attempt(&context, AttemptStatus::Succeeded, artifacts)
                    .await?;
                Ok(())
            }
            Err(err) => {
                warn!(
                    task_id = %context.task.id,
                    attempt_id = %context.attempt.id,
                    error = %err,
                    "Attempt execution failed"
                );
                self.fail_attempt(&context, &err).await;
                Err(err)
            }
        }
    }

    async fn list_tasks(&self, status: TaskStatus) -> Result<Vec<TaskListResponse>> {
        let response = self
            .send_authenticated(|client, base| {
                client
                    .get(format!("{base}/tasks"))
                    .query(&[("status", status)])
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to list tasks: {} - {}", status, body));
        }

        let tasks: Vec<TaskListResponse> = parse_json(response).await?;
        Ok(tasks)
    }

    async fn start_attempt(&self, task: TaskListResponse) -> Result<Option<AttemptContext>> {
        let response = self
            .send_authenticated(|client, base| {
                client.post(format!("{base}/tasks/{}/claim", task.id))
            })
            .await?;

        match response.status() {
            StatusCode::CONFLICT => {
                info!(task_id = %task.id, "Task already claimed by another worker");
                return Ok(None);
            }
            status if !status.is_success() => {
                let body = response.text().await.unwrap_or_default();
                return Err(anyhow!("Failed to claim task: {} - {}", status, body));
            }
            _ => {}
        }

        let environment_id = task
            .environment_id
            .clone()
            .or_else(|| self.config().environment_id.clone());

        let response = self
            .send_authenticated(|client, base| {
                client
                    .post(format!("{base}/tasks/{}/attempts", task.id))
                    .json(&AttemptCreateRequest {
                        environment_id: environment_id.clone(),
                    })
            })
            .await?;

        if response.status() == StatusCode::FORBIDDEN {
            info!(task_id = %task.id, "Current user is not the assignee, skipping");
            return Ok(None);
        }

        if response.status() == StatusCode::CONFLICT {
            info!(task_id = %task.id, "Attempt already running for task");
            return Ok(None);
        }

        let attempt: AttemptRead = parse_json(response).await?;
        info!(attempt_id = %attempt.id, task_id = %task.id, "Attempt started");

        let detail = match self.fetch_task_detail(task.id).await {
            Ok(detail) => detail,
            Err(err) => {
                warn!(task_id = %task.id, error = %err, "Failed to fetch task detail");
                None
            }
        };

        Ok(Some(AttemptContext {
            task,
            attempt,
            detail,
        }))
    }

    async fn run_attempt(&self, context: &AttemptContext) -> Result<AttemptArtifacts> {
        let timestamp = Utc::now().to_rfc3339();
        let detail = context.detail.as_ref();

        let diff = build_placeholder_diff(&context.task, detail, &timestamp);
        let log = build_execution_log(context, &timestamp);

        Ok(AttemptArtifacts {
            diff: Some(diff),
            log: Some(log),
        })
    }

    async fn complete_attempt(
        &self,
        context: &AttemptContext,
        status: AttemptStatus,
        artifacts: AttemptArtifacts,
    ) -> Result<()> {
        let AttemptArtifacts { diff, log } = artifacts;
        let payload = AttemptCompleteRequest { status, diff, log };

        let response = self
            .send_authenticated(|client, base| {
                client
                    .post(format!(
                        "{base}/tasks/attempts/{}/complete",
                        context.attempt.id
                    ))
                    .json(&payload)
            })
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to complete attempt: {} - {}", status, body));
        }

        Ok(())
    }

    async fn fail_attempt(&self, context: &AttemptContext, error: &AnyError) {
        let timestamp = Utc::now().to_rfc3339();
        let log = format!(
            "[{timestamp}] Attempt {} failed for task {}: {error:?}",
            context.attempt.id, context.task.id
        );

        let artifacts = AttemptArtifacts {
            diff: None,
            log: Some(log),
        };

        if let Err(err) = self
            .complete_attempt(context, AttemptStatus::Failed, artifacts)
            .await
        {
            warn!(
                task_id = %context.task.id,
                attempt_id = %context.attempt.id,
                error = %err,
                "Failed to report attempt failure"
            );
        }
    }

    async fn fetch_task_detail(&self, task_id: Uuid) -> Result<Option<TaskDetailResponse>> {
        let response = self
            .send_authenticated(|client, base| client.get(format!("{base}/tasks/{task_id}")))
            .await?;

        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            status if status.is_success() => {
                let detail: TaskDetailResponse = parse_json(response).await?;
                Ok(Some(detail))
            }
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(anyhow!(
                    "Failed to fetch task detail: {} - {}",
                    status,
                    body
                ))
            }
        }
    }

    async fn send_authenticated<F>(&self, build: F) -> Result<reqwest::Response>
    where
        F: Fn(&Client, &str) -> reqwest::RequestBuilder + Send + Sync,
    {
        let token = { self.inner.token.read().await.clone() };
        let response = build(self.client(), &self.config().api_base)
            .bearer_auth(&token)
            .send()
            .await?;

        if response.status() == StatusCode::UNAUTHORIZED {
            self.refresh_token().await?;
            let token = { self.inner.token.read().await.clone() };
            let retry = build(self.client(), &self.config().api_base)
                .bearer_auth(&token)
                .send()
                .await?;
            Ok(retry)
        } else {
            Ok(response)
        }
    }

    async fn refresh_token(&self) -> Result<()> {
        let token = Self::authenticate(self.client(), self.config()).await?;
        *self.inner.token.write().await = token;
        info!("Refreshed access token");
        Ok(())
    }

    async fn authenticate(client: &Client, config: &Config) -> Result<String> {
        let login_url = format!("{}/auth/session", config.api_base);
        let response = client
            .post(login_url)
            .json(&serde_json::json!({
                "email": config.email,
                "password": config.password,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("Authentication failed: {} - {}", status, body));
        }

        let token: TokenResponse = response.json().await?;
        Ok(token.access_token)
    }
}

async fn parse_json<T>(response: reqwest::Response) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let bytes = response.bytes().await?;
    serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "Failed to parse JSON (status {}): {}",
            status,
            String::from_utf8_lossy(&bytes)
        )
    })
}

fn build_placeholder_diff(
    task: &TaskListResponse,
    detail: Option<&TaskDetailResponse>,
    timestamp: &str,
) -> String {
    let mut diff = String::new();
    diff.push_str("diff --git a/TASK_LOG.md b/TASK_LOG.md\n");
    diff.push_str("--- a/TASK_LOG.md\n");
    diff.push_str("+++ b/TASK_LOG.md\n");
    diff.push_str("@@\n");
    diff.push_str(&format!("+## Task {} ({})\n", task.id, task.title));
    diff.push_str(&format!(
        "+Processed at {timestamp} UTC by codex-cloud-supervisor\\n"
    ));

    if let Some(detail) = detail {
        diff.push_str(&format!("+Detail ID: {}\\n", detail.id));
        diff.push_str(&format!("+Snapshot title: {}\\n", detail.title));
        if let Some(environment_id) = &detail.environment_id {
            diff.push_str(&format!("+Environment: {}\\n", environment_id));
        }
        if let Some(repository) = &detail.repository {
            diff.push_str(&format!(
                "+Repository: {} ({}) on branch {} (id {})\\n",
                repository.name, repository.git_url, repository.default_branch, repository.id
            ));
        }
        if let Some(description) = &detail.description {
            for line in description.lines() {
                diff.push_str("+> ");
                diff.push_str(line);
                diff.push('\n');
            }
        }
    }

    diff
}

fn build_execution_log(context: &AttemptContext, timestamp: &str) -> String {
    let mut log = format!(
        "[{timestamp}] Attempt {} succeeded for task {} ({})",
        context.attempt.id, context.task.id, context.task.title
    );

    if let Some(detail) = &context.detail {
        if let Some(repository) = &detail.repository {
            log.push_str(&format!(
                "\nRepository: {} ({}) default branch {} (id {})",
                repository.name, repository.git_url, repository.default_branch, repository.id
            ));
        }
        if let Some(environment_id) = &detail.environment_id {
            log.push_str(&format!("\nEnvironment: {}", environment_id));
        }
        if let Some(description) = &detail.description {
            log.push_str("\nTask description:\n");
            log.push_str(description);
        }
    }

    log
}

fn init_tracing() {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("global tracing subscriber");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path, path_regex, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn supervisor_processes_pending_task_and_reports_artifacts() {
        let server = MockServer::start().await;
        let task_id = Uuid::new_v4();
        let attempt_id = Uuid::new_v4();
        let repository_id = Uuid::new_v4();

        Mock::given(method("POST"))
            .and(path("/auth/session"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "test-token"
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/tasks"))
            .and(query_param("status", "pending"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "id": task_id,
                    "title": "Demo Task"
                }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path(format!("/tasks/{task_id}/claim")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "claim_expires_at": "2024-01-01T00:00:00Z"
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path(format!("/tasks/{task_id}/attempts")))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "id": attempt_id
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(format!("/tasks/{task_id}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": task_id,
                "title": "Demo Task",
                "description": "Automated executor demo",
                "environment_id": "local-dev",
                "repository": {
                    "id": repository_id,
                    "name": "demo-repo",
                    "git_url": "https://example.com/demo.git",
                    "default_branch": "main"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path_regex(r"/tasks/attempts/.*/complete"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "succeeded",
                "diff_url": null,
                "log_url": null
            })))
            .expect(1)
            .mount(&server)
            .await;

        let config = Config {
            api_base: server.uri(),
            email: "worker@example.com".into(),
            password: "password".into(),
            poll_interval: Duration::from_secs(1),
            environment_id: None,
            max_concurrency: 1,
        };

        let supervisor = Supervisor::new(config).await.expect("supervisor init");
        supervisor
            .process_pending_tasks()
            .await
            .expect("process pending tasks");

        let requests = server
            .received_requests()
            .await
            .expect("request recording enabled");
        let complete_request = requests
            .iter()
            .find(|request| {
                request.url.path() == format!("/tasks/attempts/{}/complete", attempt_id)
            })
            .expect("complete request present");

        let body: serde_json::Value = complete_request.body_json().expect("json body");
        assert_eq!(body["status"], "succeeded");

        let diff = body["diff"].as_str().expect("diff text present");
        assert!(diff.contains(&task_id.to_string()));
        assert!(diff.contains("Demo Task"));
        assert!(diff.contains("demo-repo"));

        let log = body["log"].as_str().expect("log text present");
        assert!(log.contains(&attempt_id.to_string()));
        assert!(log.contains("Demo Task"));
        assert!(log.contains("demo-repo"));
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let args = Args::parse();
    let supervisor = Supervisor::new(args.into()).await?;
    if let Err(err) = supervisor.run().await {
        error!(error = %err, "Supervisor exited with error");
        return Err(err);
    }
    Ok(())
}
