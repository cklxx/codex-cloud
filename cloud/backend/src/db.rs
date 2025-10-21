use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Executor, SqlitePool};
use std::str::FromStr;

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

    Ok(())
}
