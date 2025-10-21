use std::str::FromStr;

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::routing::{get, post};
use chrono::Utc;
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::artifacts;
use crate::error::AppError;
use crate::models::{
    AttemptCompleteRequest, AttemptCompleteResponse, AttemptRead, AttemptStatus, ClaimResponse,
    CreateUserRequest, CreateUserResponse, LoginRequest, Repository, RepositoryCreate,
    RepositoryRead, Task, TaskAttempt, TaskCreate, TaskDetail, TaskListResponse, TaskStatus, User,
    claim_expiration, format_datetime, parse_datetime,
};
use crate::security::{CurrentUser, create_access_token, hash_password, verify_password};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct TaskFilter {
    status: Option<TaskStatus>,
}

pub fn app_router(state: AppState) -> Router {
    let cors_layer = if state.config.allow_all_cors() {
        CorsLayer::permissive()
    } else {
        let origins = state
            .config
            .cors_origins()
            .into_iter()
            .filter_map(|origin| origin.parse::<HeaderValue>().ok())
            .collect::<Vec<_>>();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any)
    };

    Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .nest("/auth", auth_routes())
        .nest("/repositories", repository_routes())
        .nest("/tasks", task_routes())
        .nest("/artifacts", artifact_routes())
        .with_state(state)
        .layer(cors_layer)
        .layer(TraceLayer::new_for_http())
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/users", post(create_user))
        .route("/session", post(login))
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), AppError> {
    let CreateUserRequest {
        email,
        password,
        name,
    } = payload;
    let password_hash = hash_password(&password)?;
    let user_id = Uuid::new_v4();
    let now = format_datetime(Utc::now());

    let result = sqlx::query(
        r#"
        INSERT INTO users (id, email, name, password_hash, auth_provider, created_at)
        VALUES (?, ?, ?, ?, 'local', ?)
        "#,
    )
    .bind(user_id.to_string())
    .bind(&email)
    .bind(&name)
    .bind(password_hash)
    .bind(now)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            let user = User {
                id: user_id,
                email,
                name,
            };
            Ok((StatusCode::CREATED, Json(CreateUserResponse::from(user))))
        }
        Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE") => {
            Err(AppError::conflict("User already exists"))
        }
        Err(err) => Err(err.into()),
    }
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<crate::models::TokenResponse>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id, password_hash, name
        FROM users
        WHERE email = ?
        "#,
    )
    .bind(&payload.email)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or_else(|| AppError::unauthorized("Invalid credentials"))?;
    let hashed: String = row.try_get("password_hash")?;
    if !verify_password(&payload.password, &hashed) {
        return Err(AppError::unauthorized("Invalid credentials"));
    }

    let id: String = row.try_get("id")?;
    let user_id = Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Invalid user id"))?;
    let token = create_access_token(user_id, &state.config)?;
    Ok(Json(crate::models::TokenResponse {
        access_token: token,
        token_type: "bearer".to_string(),
    }))
}

fn repository_routes() -> Router<AppState> {
    Router::new().route("/", post(create_repository).get(list_repositories))
}

async fn create_repository(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    Json(payload): Json<RepositoryCreate>,
) -> Result<(StatusCode, Json<RepositoryRead>), AppError> {
    let repository_id = Uuid::new_v4();
    let result = sqlx::query(
        r#"
        INSERT INTO repositories (id, name, git_url, default_branch)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(repository_id.to_string())
    .bind(&payload.name)
    .bind(&payload.git_url)
    .bind(&payload.default_branch)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            let repository = Repository {
                id: repository_id,
                name: payload.name,
                git_url: payload.git_url,
                default_branch: payload.default_branch,
            };
            Ok((StatusCode::CREATED, Json(RepositoryRead::from(repository))))
        }
        Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE") => {
            Err(AppError::conflict("Repository exists"))
        }
        Err(err) => Err(err.into()),
    }
}

async fn list_repositories(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
) -> Result<Json<Vec<RepositoryRead>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, git_url, default_branch
        FROM repositories
        ORDER BY name
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let repositories = rows
        .into_iter()
        .map(row_to_repository)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(RepositoryRead::from)
        .collect();

    Ok(Json(repositories))
}

fn task_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_tasks).post(create_task))
        .route("/{task_id}", get(get_task))
        .route("/{task_id}/claim", post(claim_task))
        .route("/{task_id}/attempts", post(create_attempt))
        .route("/attempts/{attempt_id}/complete", post(complete_attempt))
}

async fn list_tasks(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    Query(filter): Query<TaskFilter>,
) -> Result<Json<Vec<TaskListResponse>>, AppError> {
    let mut builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, title, description, repository_id, status, assignee_id, created_by, created_at, updated_at FROM tasks",
    );

    if let Some(status) = filter.status {
        builder.push(" WHERE status = ");
        builder.push_bind(status.as_str());
    }

    builder.push(" ORDER BY updated_at DESC");

    let rows = builder.build().fetch_all(&state.pool).await?;

    let tasks = rows
        .into_iter()
        .map(row_to_task)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(TaskListResponse::from)
        .collect();

    Ok(Json(tasks))
}

async fn create_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(payload): Json<TaskCreate>,
) -> Result<(StatusCode, Json<TaskDetail>), AppError> {
    let repository_exists =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(1) FROM repositories WHERE id = ?")
            .bind(payload.repository_id.to_string())
            .fetch_one(&state.pool)
            .await?;

    if repository_exists == 0 {
        return Err(AppError::bad_request("Repository not found"));
    }

    let task_id = Uuid::new_v4();
    let now = Utc::now();
    let now_str = format_datetime(now);

    sqlx::query(
        r#"
        INSERT INTO tasks (id, title, description, repository_id, status, assignee_id, created_by, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?)
        "#,
    )
    .bind(task_id.to_string())
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(payload.repository_id.to_string())
    .bind(TaskStatus::Pending.as_str())
    .bind(user.id.to_string())
    .bind(now_str.clone())
    .bind(now_str.clone())
    .execute(&state.pool)
    .await?;

    let repository = fetch_repository(&state.pool, payload.repository_id).await?;

    let task = Task {
        id: task_id,
        title: payload.title,
        description: payload.description,
        repository_id: payload.repository_id,
        status: TaskStatus::Pending,
        assignee_id: None,
        created_by: user.id,
        created_at: now,
        updated_at: now,
    };

    Ok((
        StatusCode::CREATED,
        Json(TaskDetail::from_entities(
            task,
            Some(repository),
            Vec::<AttemptRead>::new(),
        )),
    ))
}

async fn get_task(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskDetail>, AppError> {
    let task = fetch_task(&state.pool, task_id).await?;
    let repository = fetch_repository(&state.pool, task.repository_id).await.ok();
    let attempts = fetch_attempts(&state.pool, task.id).await?;
    let mut attempt_reads = Vec::with_capacity(attempts.len());
    for attempt in attempts {
        let diff_url =
            artifacts::artifact_url(&state.artifacts, attempt.diff_artifact_id.as_deref()).await?;
        let log_url =
            artifacts::artifact_url(&state.artifacts, attempt.log_artifact_id.as_deref()).await?;
        attempt_reads.push(AttemptRead::from_attempt(attempt, diff_url, log_url));
    }

    Ok(Json(TaskDetail::from_entities(
        task,
        repository,
        attempt_reads,
    )))
}

async fn claim_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<ClaimResponse>, AppError> {
    let mut task = fetch_task(&state.pool, task_id).await?;
    match task.status {
        TaskStatus::Pending | TaskStatus::Review => {}
        _ => return Err(AppError::conflict("Task already claimed")),
    }

    task.status = TaskStatus::Claimed;
    task.assignee_id = Some(user.id);
    task.updated_at = Utc::now();

    sqlx::query(
        r#"
        UPDATE tasks
        SET assignee_id = ?, status = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(user.id.to_string())
    .bind(task.status.as_str())
    .bind(format_datetime(task.updated_at))
    .bind(task.id.to_string())
    .execute(&state.pool)
    .await?;

    Ok(Json(ClaimResponse {
        claim_expires_at: claim_expiration(30),
    }))
}

async fn create_attempt(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(task_id): Path<Uuid>,
    Json(_payload): Json<crate::models::AttemptCreate>,
) -> Result<(StatusCode, Json<AttemptRead>), AppError> {
    let mut task = fetch_task(&state.pool, task_id).await?;
    if task.assignee_id != Some(user.id) {
        return Err(AppError::forbidden("Task must be claimed"));
    }

    task.status = TaskStatus::Running;
    task.updated_at = Utc::now();

    let attempt_id = Uuid::new_v4();
    let now = Utc::now();
    let now_str = format_datetime(now);

    sqlx::query(
        r#"
        INSERT INTO task_attempts (id, task_id, created_by, status, diff_artifact_id, log_artifact_id, created_at, updated_at)
        VALUES (?, ?, ?, ?, NULL, NULL, ?, ?)
        "#,
    )
    .bind(attempt_id.to_string())
    .bind(task.id.to_string())
    .bind(user.id.to_string())
    .bind(AttemptStatus::Running.as_str())
    .bind(now_str.clone())
    .bind(now_str.clone())
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE tasks SET status = ?, updated_at = ? WHERE id = ?
        "#,
    )
    .bind(task.status.as_str())
    .bind(format_datetime(task.updated_at))
    .bind(task.id.to_string())
    .execute(&state.pool)
    .await?;

    let attempt = TaskAttempt {
        id: attempt_id,
        task_id: task.id,
        created_by: user.id,
        status: AttemptStatus::Running,
        diff_artifact_id: None,
        log_artifact_id: None,
        created_at: now,
        updated_at: now,
    };

    Ok((
        StatusCode::CREATED,
        Json(AttemptRead::from_attempt(attempt, None, None)),
    ))
}

async fn complete_attempt(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<AttemptCompleteRequest>,
) -> Result<Json<AttemptCompleteResponse>, AppError> {
    let mut attempt = fetch_attempt(&state.pool, attempt_id).await?;
    let mut task = fetch_task(&state.pool, attempt.task_id).await?;

    if task.assignee_id != Some(user.id) {
        return Err(AppError::forbidden("Not assigned to task"));
    }

    if let Some(diff) = payload.diff.as_ref() {
        attempt.diff_artifact_id =
            Some(artifacts::store_text_artifact(&state.artifacts, diff, "diff").await?);
    }
    if let Some(log) = payload.log.as_ref() {
        attempt.log_artifact_id =
            Some(artifacts::store_text_artifact(&state.artifacts, log, "log").await?);
    }

    attempt.status = payload.status;
    attempt.updated_at = Utc::now();
    task.updated_at = attempt.updated_at;

    match attempt.status {
        AttemptStatus::Succeeded => {
            task.status = TaskStatus::Review;
        }
        AttemptStatus::Failed => {
            task.status = TaskStatus::Pending;
        }
        _ => {}
    }

    sqlx::query(
        r#"
        UPDATE task_attempts
        SET status = ?, diff_artifact_id = ?, log_artifact_id = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(attempt.status.as_str())
    .bind(&attempt.diff_artifact_id)
    .bind(&attempt.log_artifact_id)
    .bind(format_datetime(attempt.updated_at))
    .bind(attempt.id.to_string())
    .execute(&state.pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE tasks SET status = ?, updated_at = ? WHERE id = ?
        "#,
    )
    .bind(task.status.as_str())
    .bind(format_datetime(task.updated_at))
    .bind(task.id.to_string())
    .execute(&state.pool)
    .await?;

    Ok(Json(AttemptCompleteResponse {
        status: attempt.status,
        diff_url: artifacts::artifact_url(&state.artifacts, attempt.diff_artifact_id.as_deref())
            .await?,
        log_url: artifacts::artifact_url(&state.artifacts, attempt.log_artifact_id.as_deref())
            .await?,
    }))
}

fn artifact_routes() -> Router<AppState> {
    Router::new().route("/{artifact_id}", get(get_artifact))
}

async fn get_artifact(
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<(StatusCode, String), AppError> {
    let content = artifacts::read_artifact(&state.artifacts, &artifact_id).await?;
    Ok((StatusCode::OK, content))
}

async fn fetch_repository(pool: &SqlitePool, id: Uuid) -> Result<Repository, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id, name, git_url, default_branch
        FROM repositories
        WHERE id = ?
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or_else(|| AppError::bad_request("Repository not found"))?;
    row_to_repository(row)
}

async fn fetch_task(pool: &SqlitePool, id: Uuid) -> Result<Task, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id, title, description, repository_id, status, assignee_id, created_by, created_at, updated_at
        FROM tasks
        WHERE id = ?
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or_else(|| AppError::not_found("Task not found"))?;
    row_to_task(row)
}

async fn fetch_attempt(pool: &SqlitePool, id: Uuid) -> Result<TaskAttempt, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id, task_id, created_by, status, diff_artifact_id, log_artifact_id, created_at, updated_at
        FROM task_attempts
        WHERE id = ?
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or_else(|| AppError::not_found("Attempt not found"))?;
    row_to_attempt(row)
}

async fn fetch_attempts(pool: &SqlitePool, task_id: Uuid) -> Result<Vec<TaskAttempt>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT id, task_id, created_by, status, diff_artifact_id, log_artifact_id, created_at, updated_at
        FROM task_attempts
        WHERE task_id = ?
        ORDER BY created_at DESC
        "#,
    )
    .bind(task_id.to_string())
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(row_to_attempt).collect()
}

fn row_to_repository(row: SqliteRow) -> Result<Repository, AppError> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let git_url: String = row.try_get("git_url")?;
    let default_branch: String = row.try_get("default_branch")?;
    Ok(Repository {
        id: parse_uuid(&id, "repository id")?,
        name,
        git_url,
        default_branch,
    })
}

fn row_to_task(row: SqliteRow) -> Result<Task, AppError> {
    let id: String = row.try_get("id")?;
    let title: String = row.try_get("title")?;
    let description: Option<String> = row.try_get("description")?;
    let repository_id: String = row.try_get("repository_id")?;
    let status: String = row.try_get("status")?;
    let assignee_id: Option<String> = row.try_get("assignee_id")?;
    let created_by: String = row.try_get("created_by")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;

    Ok(Task {
        id: parse_uuid(&id, "task id")?,
        title,
        description,
        repository_id: parse_uuid(&repository_id, "repository id")?,
        status: TaskStatus::from_str(&status)?,
        assignee_id: parse_optional_uuid(assignee_id, "assignee id")?,
        created_by: parse_uuid(&created_by, "created by")?,
        created_at: parse_datetime(&created_at)?,
        updated_at: parse_datetime(&updated_at)?,
    })
}

fn row_to_attempt(row: SqliteRow) -> Result<TaskAttempt, AppError> {
    let id: String = row.try_get("id")?;
    let task_id: String = row.try_get("task_id")?;
    let created_by: String = row.try_get("created_by")?;
    let status: String = row.try_get("status")?;
    let diff_artifact_id: Option<String> = row.try_get("diff_artifact_id")?;
    let log_artifact_id: Option<String> = row.try_get("log_artifact_id")?;
    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;

    Ok(TaskAttempt {
        id: parse_uuid(&id, "attempt id")?,
        task_id: parse_uuid(&task_id, "task id")?,
        created_by: parse_uuid(&created_by, "created by")?,
        status: AttemptStatus::from_str(&status)?,
        diff_artifact_id,
        log_artifact_id,
        created_at: parse_datetime(&created_at)?,
        updated_at: parse_datetime(&updated_at)?,
    })
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|_| AppError::bad_request(format!("Invalid {field}")))
}

fn parse_optional_uuid(value: Option<String>, field: &str) -> Result<Option<Uuid>, AppError> {
    value.map(|id| parse_uuid(&id, field)).transpose()
}
