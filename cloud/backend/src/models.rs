use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Claimed,
    Running,
    Review,
    Applied,
}

pub fn format_datetime(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

pub fn parse_datetime(value: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| AppError::bad_request(format!("Invalid datetime: {value}")))
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Claimed => "claimed",
            Self::Running => "running",
            Self::Review => "review",
            Self::Applied => "applied",
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TaskStatus {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "claimed" => Ok(Self::Claimed),
            "running" => Ok(Self::Running),
            "review" => Ok(Self::Review),
            "applied" => Ok(Self::Applied),
            other => Err(AppError::bad_request(format!(
                "Invalid task status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttemptStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl AttemptStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for AttemptStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AttemptStatus {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            other => Err(AppError::bad_request(format!(
                "Invalid attempt status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Repository {
    pub id: Uuid,
    pub name: String,
    pub git_url: String,
    pub default_branch: String,
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub id: String,
    pub label: Option<String>,
    pub repository_id: Uuid,
    pub branch: String,
    pub is_pinned: bool,
    pub provider: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub repository_id: Uuid,
    pub status: TaskStatus,
    pub assignee_id: Option<Uuid>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub environment_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskAttempt {
    pub id: Uuid,
    pub task_id: Uuid,
    pub created_by: Uuid,
    pub status: AttemptStatus,
    pub diff_artifact_id: Option<String>,
    pub log_artifact_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserResponse {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
}

impl From<User> for CreateUserResponse {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
            name: value.name,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default = "default_token_type")]
    pub token_type: String,
}

fn default_token_type() -> String {
    "bearer".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryCreate {
    pub name: String,
    pub git_url: String,
    pub default_branch: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryRead {
    pub id: Uuid,
    pub name: String,
    pub git_url: String,
    pub default_branch: String,
}

impl From<Repository> for RepositoryRead {
    fn from(value: Repository) -> Self {
        Self {
            id: value.id,
            name: value.name,
            git_url: value.git_url,
            default_branch: value.default_branch,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvironmentCreate {
    pub id: String,
    pub label: Option<String>,
    pub repository_id: Uuid,
    pub branch: String,
    #[serde(default)]
    pub is_pinned: bool,
    pub provider: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvironmentRead {
    pub id: String,
    pub label: Option<String>,
    pub repository_id: Uuid,
    pub branch: String,
    pub is_pinned: bool,
    pub provider: Option<String>,
    pub owner: Option<String>,
    pub repo: Option<String>,
}

impl From<Environment> for EnvironmentRead {
    fn from(value: Environment) -> Self {
        Self {
            id: value.id,
            label: value.label,
            repository_id: value.repository_id,
            branch: value.branch,
            is_pinned: value.is_pinned,
            provider: value.provider,
            owner: value.owner,
            repo: value.repo,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskCreate {
    pub title: String,
    pub description: Option<String>,
    pub repository_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskRead {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub repository_id: Uuid,
    pub assignee_id: Option<Uuid>,
    pub created_by: Uuid,
    pub updated_at: DateTime<Utc>,
    pub environment_id: Option<String>,
}

impl From<Task> for TaskRead {
    fn from(value: Task) -> Self {
        Self {
            id: value.id,
            title: value.title,
            description: value.description,
            status: value.status,
            repository_id: value.repository_id,
            assignee_id: value.assignee_id,
            created_by: value.created_by,
            updated_at: value.updated_at,
            environment_id: value.environment_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskListResponse {
    pub id: Uuid,
    pub title: String,
    pub status: TaskStatus,
    pub repository_id: Uuid,
    pub updated_at: DateTime<Utc>,
    pub environment_id: Option<String>,
}

impl From<Task> for TaskListResponse {
    fn from(value: Task) -> Self {
        Self {
            id: value.id,
            title: value.title,
            status: value.status,
            repository_id: value.repository_id,
            updated_at: value.updated_at,
            environment_id: value.environment_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaimResponse {
    pub claim_expires_at: DateTime<Utc>,
}

pub fn claim_expiration(minutes: i64) -> DateTime<Utc> {
    Utc::now() + Duration::minutes(minutes)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttemptCreate {
    pub environment_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttemptRead {
    pub id: Uuid,
    pub task_id: Uuid,
    pub status: AttemptStatus,
    pub diff_artifact_id: Option<String>,
    pub diff_url: Option<String>,
    pub log_artifact_id: Option<String>,
    pub log_url: Option<String>,
    pub created_by: Uuid,
    pub updated_at: DateTime<Utc>,
}

impl From<TaskAttempt> for AttemptRead {
    fn from(value: TaskAttempt) -> Self {
        Self {
            id: value.id,
            task_id: value.task_id,
            status: value.status,
            diff_artifact_id: value.diff_artifact_id,
            diff_url: None,
            log_artifact_id: value.log_artifact_id,
            log_url: None,
            created_by: value.created_by,
            updated_at: value.updated_at,
        }
    }
}

impl AttemptRead {
    pub fn from_attempt(
        attempt: TaskAttempt,
        diff_url: Option<String>,
        log_url: Option<String>,
    ) -> Self {
        let mut read = Self::from(attempt);
        read.diff_url = diff_url;
        read.log_url = log_url;
        read
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttemptCompleteRequest {
    pub status: AttemptStatus,
    pub diff: Option<String>,
    pub log: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttemptCompleteResponse {
    pub status: AttemptStatus,
    pub diff_url: Option<String>,
    pub log_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskDetail {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub repository_id: Uuid,
    pub assignee_id: Option<Uuid>,
    pub created_by: Uuid,
    pub updated_at: DateTime<Utc>,
    pub environment_id: Option<String>,
    pub repository: Option<RepositoryRead>,
    pub attempts: Vec<AttemptRead>,
}

impl TaskDetail {
    pub fn from_entities(
        task: Task,
        repository: Option<Repository>,
        attempts: Vec<AttemptRead>,
    ) -> Self {
        let repository = repository.map(RepositoryRead::from);
        Self {
            id: task.id,
            title: task.title,
            description: task.description,
            status: task.status,
            repository_id: task.repository_id,
            assignee_id: task.assignee_id,
            created_by: task.created_by,
            updated_at: task.updated_at,
            environment_id: task.environment_id,
            repository,
            attempts,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodexEnvironmentSummary {
    pub id: String,
    pub label: Option<String>,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CodexTaskCreate {
    pub new_task: CodexNewTask,
    #[serde(default)]
    pub input_items: Vec<CodexInputItem>,
    #[serde(default)]
    pub metadata: Option<CodexTaskMetadata>,
}

#[derive(Debug, Deserialize)]
pub struct CodexNewTask {
    pub environment_id: String,
    pub branch: Option<String>,
    #[serde(default)]
    pub run_environment_in_qa_mode: bool,
}

#[derive(Debug, Deserialize)]
pub struct CodexInputItem {
    #[serde(rename = "type")]
    pub kind: String,
    pub role: Option<String>,
    #[serde(default)]
    pub content: Vec<CodexInputContent>,
}

#[derive(Debug, Deserialize)]
pub struct CodexInputContent {
    #[serde(rename = "content_type")]
    pub content_type: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CodexTaskMetadata {
    #[serde(default)]
    pub best_of_n: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodexTaskCreateResponse {
    pub task: CodexCreatedTask,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodexCreatedTask {
    pub id: Uuid,
    pub status: TaskStatus,
    pub environment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_total: Option<usize>,
}
