use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Executor, Row, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

use crate::models::User;

pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(database_url)?.create_if_missing(true);
    SqlitePoolOptions::new().connect_with(options).await
}

pub async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    pool.execute("PRAGMA foreign_keys = ON").await?;
    pool.execute(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            name TEXT,
            password_hash TEXT NOT NULL,
            auth_provider TEXT NOT NULL,
            created_at TEXT NOT NULL
        )
        "#,
    )
    .await?;

    pool.execute(
        r#"
        CREATE TABLE IF NOT EXISTS repositories (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            git_url TEXT NOT NULL UNIQUE,
            default_branch TEXT NOT NULL
        )
        "#,
    )
    .await?;

    pool.execute(
        r#"
        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            repository_id TEXT NOT NULL,
            status TEXT NOT NULL,
            assignee_id TEXT,
            created_by TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            environment_id TEXT,
            FOREIGN KEY(repository_id) REFERENCES repositories(id),
            FOREIGN KEY(assignee_id) REFERENCES users(id),
            FOREIGN KEY(created_by) REFERENCES users(id)
        )
        "#,
    )
    .await?;

    pool.execute(
        r#"
        CREATE TABLE IF NOT EXISTS task_attempts (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            created_by TEXT NOT NULL,
            status TEXT NOT NULL,
            diff_artifact_id TEXT,
            log_artifact_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(task_id) REFERENCES tasks(id),
            FOREIGN KEY(created_by) REFERENCES users(id)
        )
        "#,
    )
    .await?;

    pool.execute(
        r#"
        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)
        "#,
    )
    .await?;

    // Environment catalog used by Codex CLI compatibility endpoints.
    pool.execute(
        r#"
        CREATE TABLE IF NOT EXISTS environments (
            id TEXT PRIMARY KEY,
            label TEXT,
            repository_id TEXT NOT NULL,
            branch TEXT NOT NULL,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            provider TEXT,
            owner TEXT,
            repo TEXT,
            FOREIGN KEY(repository_id) REFERENCES repositories(id)
        )
        "#,
    )
    .await?;

    pool.execute(
        r#"
        CREATE TABLE IF NOT EXISTS external_identities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            issuer TEXT NOT NULL,
            subject TEXT NOT NULL,
            user_id TEXT NOT NULL,
            email TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(issuer, subject),
            FOREIGN KEY(user_id) REFERENCES users(id)
        )
        "#,
    )
    .await?;

    // Backfill environment_id column for existing databases; ignore the error
    // when the column already exists.
    let _ = pool
        .execute("ALTER TABLE tasks ADD COLUMN environment_id TEXT")
        .await;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct ExternalIdentitySeed<'a> {
    pub issuer: &'a str,
    pub subject: &'a str,
    pub user_id: Uuid,
    pub email: Option<&'a str>,
}

pub async fn seed_external_identities(
    pool: &SqlitePool,
    seeds: &[ExternalIdentitySeed<'_>],
) -> Result<(), sqlx::Error> {
    for seed in seeds {
        let timestamp = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO external_identities (issuer, subject, user_id, email, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(issuer, subject) DO UPDATE SET
                user_id = excluded.user_id,
                email = excluded.email,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(seed.issuer)
        .bind(seed.subject)
        .bind(seed.user_id.to_string())
        .bind(seed.email)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn find_user_by_external_identity(
    pool: &SqlitePool,
    issuer: &str,
    subject: &str,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT u.id as user_id, u.email, u.name
        FROM external_identities ei
        JOIN users u ON u.id = ei.user_id
        WHERE ei.issuer = ? AND ei.subject = ?
        "#,
    )
    .bind(issuer)
    .bind(subject)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = row {
        let id_str: String = row.try_get("user_id")?;
        let id = Uuid::parse_str(&id_str).map_err(|err| sqlx::Error::ColumnDecode {
            index: "user_id".to_string(),
            source: Box::new(err),
        })?;
        let email: String = row.try_get("email")?;
        let name: Option<String> = row.try_get("name")?;
        return Ok(Some(User { id, email, name }));
    }

    Ok(None)
}
