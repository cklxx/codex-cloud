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
use crate::db;
use crate::error::AppError;
use crate::models::{
    AttemptCompleteRequest, AttemptCompleteResponse, AttemptRead, AttemptStatus, ClaimResponse,
    CodexEnvironmentSummary, CodexInputItem, CodexTaskCreate, CodexTaskCreateResponse,
    CreateUserRequest, CreateUserResponse, Environment, EnvironmentCreate, EnvironmentRead,
    LoginRequest, Repository, RepositoryCreate, RepositoryRead, Task, TaskAttempt, TaskCreate,
    TaskDetail, TaskListResponse, TaskStatus, User, claim_expiration, format_datetime,
    parse_datetime,
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
        .nest("/environments", environment_routes())
        .nest("/tasks", task_routes())
        .nest("/artifacts", artifact_routes())
        .nest("/api/codex", codex_routes())
        .with_state(state)
        .layer(cors_layer)
        .layer(TraceLayer::new_for_http())
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/users", post(create_user))
        .route("/session", post(login))
        .route("/oidc/callback", get(oidc_callback))
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

#[derive(Debug, Deserialize)]
struct OidcCallbackQuery {
    code: String,
}

async fn oidc_callback(
    State(state): State<AppState>,
    Query(query): Query<OidcCallbackQuery>,
) -> Result<Json<crate::models::TokenResponse>, AppError> {
    let provider = state
        .oidc
        .as_ref()
        .ok_or_else(|| AppError::bad_request("OpenID Connect not configured"))?;

    let id_token = provider.exchange_code(&query.code).await?;
    let claims = provider.validate_id_token(&id_token).await?;
    let user = db::find_user_by_external_identity(&state.pool, provider.issuer(), &claims.subject)
        .await?
        .ok_or_else(|| AppError::unauthorized("No account linked to external identity"))?;

    let token = create_access_token(user.id, &state.config)?;
    Ok(Json(crate::models::TokenResponse {
        access_token: token,
        token_type: "bearer".to_string(),
    }))
}

fn repository_routes() -> Router<AppState> {
    Router::new().route("/", post(create_repository).get(list_repositories))
}

fn environment_routes() -> Router<AppState> {
    Router::new().route("/", post(create_environment))
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

async fn create_environment(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    Json(payload): Json<EnvironmentCreate>,
) -> Result<(StatusCode, Json<EnvironmentRead>), AppError> {
    let EnvironmentCreate {
        id,
        label,
        repository_id,
        branch,
        is_pinned,
        provider,
        owner,
        repo,
    } = payload;

    let repository = fetch_repository(&state.pool, repository_id).await?;

    let provider = provider.map(|p| p.to_lowercase());
    let owner = owner.map(|o| o.to_lowercase());
    let repo = repo.map(|r| r.to_lowercase());
    let (provider, owner, repo) = match (provider, owner, repo) {
        (Some(p), Some(o), Some(r)) => (Some(p), Some(o), Some(r)),
        _ => match parse_repository_coordinates(&repository.git_url) {
            Some((p, o, r)) => (Some(p), Some(o), Some(r)),
            None => (None, None, None),
        },
    };

    let result = sqlx::query(
        r#"
        INSERT INTO environments (id, label, repository_id, branch, is_pinned, provider, owner, repo)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&label)
    .bind(repository_id.to_string())
    .bind(&branch)
    .bind(if is_pinned { 1 } else { 0 })
    .bind(&provider)
    .bind(&owner)
    .bind(&repo)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            let environment = Environment {
                id,
                label,
                repository_id,
                branch,
                is_pinned,
                provider,
                owner,
                repo,
            };
            Ok((
                StatusCode::CREATED,
                Json(EnvironmentRead::from(environment)),
            ))
        }
        Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE") => {
            Err(AppError::conflict("Environment already exists"))
        }
        Err(err) => Err(err.into()),
    }
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
        "SELECT id, title, description, repository_id, status, assignee_id, created_by, created_at, updated_at, environment_id FROM tasks",
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
        INSERT INTO tasks (id, title, description, repository_id, status, assignee_id, created_by, created_at, updated_at, environment_id)
        VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?, ?)
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
    .bind(Option::<String>::None)
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
        environment_id: None,
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

fn codex_routes() -> Router<AppState> {
    Router::new()
        .route("/environments", get(list_codex_environments))
        .route(
            "/environments/by-repo/{provider}/{owner}/{repo}",
            get(list_codex_environments_by_repo),
        )
        .route("/tasks", post(create_codex_task))
}

async fn get_artifact(
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<(StatusCode, String), AppError> {
    let content = artifacts::read_artifact(&state.artifacts, &artifact_id).await?;
    Ok((StatusCode::OK, content))
}

async fn list_codex_environments(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
) -> Result<Json<Vec<CodexEnvironmentSummary>>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT id, label, repository_id, branch, is_pinned, provider, owner, repo,
               (SELECT COUNT(1) FROM tasks WHERE environment_id = environments.id) AS task_count
        FROM environments
        ORDER BY is_pinned DESC, COALESCE(label, id)
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    let environments = rows
        .into_iter()
        .map(row_to_environment_with_count)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(environment, count)| codex_environment_summary(environment, Some(count)))
        .collect();

    Ok(Json(environments))
}

async fn list_codex_environments_by_repo(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    Path((provider, owner, repo)): Path<(String, String, String)>,
) -> Result<Json<Vec<CodexEnvironmentSummary>>, AppError> {
    let provider = provider.to_lowercase();
    let owner = owner.to_lowercase();
    let repo = repo.to_lowercase();

    let rows = sqlx::query(
        r#"
        SELECT id, label, repository_id, branch, is_pinned, provider, owner, repo,
               (SELECT COUNT(1) FROM tasks WHERE environment_id = environments.id) AS task_count
        FROM environments
        WHERE provider = ? AND owner = ? AND repo = ?
        ORDER BY is_pinned DESC, COALESCE(label, id)
        "#,
    )
    .bind(&provider)
    .bind(&owner)
    .bind(&repo)
    .fetch_all(&state.pool)
    .await?;

    let environments = rows
        .into_iter()
        .map(row_to_environment_with_count)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(environment, count)| codex_environment_summary(environment, Some(count)))
        .collect();

    Ok(Json(environments))
}

async fn create_codex_task(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(payload): Json<CodexTaskCreate>,
) -> Result<(StatusCode, Json<CodexTaskCreateResponse>), AppError> {
    let CodexTaskCreate {
        new_task,
        input_items,
        metadata,
    } = payload;

    let environment = fetch_environment(&state.pool, &new_task.environment_id).await?;
    let prompt = extract_codex_prompt(&input_items)?;
    let title = derive_codex_title(&prompt);

    let task_id = Uuid::new_v4();
    let now = Utc::now();
    let now_str = format_datetime(now);

    sqlx::query(
        r#"
        INSERT INTO tasks (id, title, description, repository_id, status, assignee_id, created_by, created_at, updated_at, environment_id)
        VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?, ?)
        "#,
    )
    .bind(task_id.to_string())
    .bind(&title)
    .bind(Some(prompt))
    .bind(environment.repository_id.to_string())
    .bind(TaskStatus::Pending.as_str())
    .bind(user.id.to_string())
    .bind(now_str.clone())
    .bind(now_str.clone())
    .bind(Some(environment.id.clone()))
    .execute(&state.pool)
    .await?;

    let response = CodexTaskCreateResponse {
        task: crate::models::CodexCreatedTask {
            id: task_id,
            status: TaskStatus::Pending,
            environment_id: Some(environment.id),
            attempt_total: metadata.and_then(|meta| meta.best_of_n),
        },
    };

    Ok((StatusCode::CREATED, Json(response)))
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
        SELECT id, title, description, repository_id, status, assignee_id, created_by, created_at, updated_at, environment_id
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
    let environment_id: Option<String> = row.try_get("environment_id")?;

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
        environment_id,
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

fn row_to_environment(row: SqliteRow) -> Result<Environment, AppError> {
    let id: String = row.try_get("id")?;
    let label: Option<String> = row.try_get("label")?;
    let repository_id: String = row.try_get("repository_id")?;
    let branch: String = row.try_get("branch")?;
    let is_pinned: i64 = row.try_get("is_pinned")?;
    let provider: Option<String> = row.try_get("provider")?;
    let owner: Option<String> = row.try_get("owner")?;
    let repo: Option<String> = row.try_get("repo")?;

    Ok(Environment {
        id,
        label,
        repository_id: parse_uuid(&repository_id, "repository id")?,
        branch,
        is_pinned: is_pinned != 0,
        provider,
        owner,
        repo,
    })
}

fn row_to_environment_with_count(row: SqliteRow) -> Result<(Environment, i64), AppError> {
    let count: i64 = row.try_get("task_count")?;
    let environment = row_to_environment(row)?;
    Ok((environment, count))
}

fn codex_environment_summary(
    environment: Environment,
    count: Option<i64>,
) -> CodexEnvironmentSummary {
    CodexEnvironmentSummary {
        id: environment.id,
        label: environment.label,
        is_pinned: environment.is_pinned,
        task_count: count,
    }
}

async fn fetch_environment(pool: &SqlitePool, id: &str) -> Result<Environment, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id, label, repository_id, branch, is_pinned, provider, owner, repo
        FROM environments
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let row = row.ok_or_else(|| AppError::bad_request("Environment not found"))?;
    row_to_environment(row)
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(|_| AppError::bad_request(format!("Invalid {field}")))
}

fn parse_optional_uuid(value: Option<String>, field: &str) -> Result<Option<Uuid>, AppError> {
    value.map(|id| parse_uuid(&id, field)).transpose()
}

fn derive_codex_title(prompt: &str) -> String {
    let title = prompt
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or("Codex Cloud task");

    let mut collected: String = title.chars().take(80).collect();
    if collected.is_empty() {
        collected = "Codex Cloud task".to_string();
    }
    collected
}

fn extract_codex_prompt(items: &[CodexInputItem]) -> Result<String, AppError> {
    let mut segments: Vec<String> = Vec::new();
    for item in items {
        if item.kind != "message" {
            continue;
        }
        if let Some(role) = &item.role
            && !role.eq_ignore_ascii_case("user")
        {
            continue;
        }
        for fragment in &item.content {
            if fragment
                .content_type
                .as_deref()
                .map(|ct| ct.eq_ignore_ascii_case("text"))
                .unwrap_or(true)
            {
                if let Some(text) = fragment.text.as_ref() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        segments.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    let prompt = segments.join("\n\n");
    if prompt.trim().is_empty() {
        Err(AppError::bad_request("Prompt content is required"))
    } else {
        Ok(prompt)
    }
}

fn parse_repository_coordinates(git_url: &str) -> Option<(String, String, String)> {
    let trimmed = git_url.trim();
    let mut normalized = trimmed.trim_end_matches('/').to_string();
    normalized = normalized.trim_end_matches(".git").to_string();

    let github_prefixes = [
        "https://github.com/",
        "http://github.com/",
        "https://www.github.com/",
        "http://www.github.com/",
    ];

    if let Some(rest) = normalized.strip_prefix("git@github.com:") {
        return split_repo_slug(rest).map(|(owner, repo)| ("github".to_string(), owner, repo));
    }

    for prefix in &github_prefixes {
        if let Some(rest) = normalized.strip_prefix(prefix) {
            return split_repo_slug(rest).map(|(owner, repo)| ("github".to_string(), owner, repo));
        }
    }

    None
}

fn split_repo_slug(input: &str) -> Option<(String, String)> {
    let mut parts = input.split('/');
    let owner = parts.next()?.trim().to_lowercase();
    let repo = parts.next()?.trim().trim_end_matches('/').to_lowercase();
    Some((owner, repo))
}
