use std::fs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::info;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use codex_cloud_backend::config::AppConfig;
use codex_cloud_backend::db;
use codex_cloud_backend::models::{CreateUserResponse, format_datetime};
use codex_cloud_backend::routes::app_router;
use codex_cloud_backend::security::hash_password;
use codex_cloud_backend::state::AppState;

#[derive(Parser, Debug)]
#[command(author, version, about = "Codex Cloud backend service")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the HTTP API server
    Serve {
        #[arg(long, default_value = "0.0.0.0:8000")]
        addr: String,
    },
    /// Create a local admin user in the database
    CreateAdmin {
        email: String,
        password: String,
        #[arg(long)]
        name: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let config = AppConfig::from_env();

    match cli.command {
        Command::Serve { addr } => serve(config, addr).await?,
        Command::CreateAdmin {
            email,
            password,
            name,
        } => create_admin(config, email, password, name).await?,
    }

    Ok(())
}

async fn serve(config: AppConfig, addr: String) -> Result<()> {
    prepare_environment(&config)?;
    let pool = db::connect(&config.database_url).await?;
    db::init_db(&pool).await?;
    let state = AppState::new(pool, config).await?;
    let app = app_router(state);

    let listener = TcpListener::bind(&addr).await?;
    info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn create_admin(
    config: AppConfig,
    email: String,
    password: String,
    name: Option<String>,
) -> Result<()> {
    prepare_environment(&config)?;
    let pool = db::connect(&config.database_url).await?;
    db::init_db(&pool).await?;

    let existing: i64 = sqlx::query_scalar("SELECT COUNT(1) FROM users WHERE email = ?")
        .bind(&email)
        .fetch_one(&pool)
        .await?;

    if existing > 0 {
        println!("User already exists");
        return Ok(());
    }

    let password_hash = hash_password(&password)?;
    let user_id = Uuid::new_v4();
    let now = format_datetime(chrono::Utc::now());

    sqlx::query(
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
    .execute(&pool)
    .await?;

    let response = CreateUserResponse {
        id: user_id,
        email,
        name,
    };
    println!("Created user: {}", response.email);
    Ok(())
}

fn prepare_environment(config: &AppConfig) -> Result<()> {
    if let Some(path) = config.database_path().and_then(|path| path.parent())
        && !path.exists()
    {
        fs::create_dir_all(path)?;
    }
    config.ensure_artifact_dir()?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
