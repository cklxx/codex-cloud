use std::time::Duration;

use codex_cloud_backend::config::AppConfig;
use codex_cloud_backend::db;
use codex_cloud_backend::routes::app_router;
use codex_cloud_backend::state::AppState;
use reqwest::Client;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

#[allow(dead_code)]
pub struct TestApp {
    pub client: Client,
    pub base_url: String,
    pub pool: sqlx::SqlitePool,
    pub config: AppConfig,
    shutdown_tx: Option<oneshot::Sender<()>>,
    _tmpdir: TempDir,
}

impl TestApp {
    #[allow(dead_code)]
    pub async fn spawn() -> Self {
        Self::spawn_with(|_| {}).await
    }

    pub async fn spawn_with<F>(configure: F) -> Self
    where
        F: FnOnce(&mut AppConfig),
    {
        let tmp = TempDir::new().unwrap();
        let artifact_dir = tmp.path().join("artifacts");
        let db_path = tmp.path().join("codex.db");

        let mut config = AppConfig {
            secret_key: "test-secret".to_string(),
            database_url: format!("sqlite://{}", db_path.display()),
            artifacts_dir: artifact_dir.clone(),
            artifact_base_url: "http://127.0.0.1:0/artifacts".to_string(),
            access_token_expire_minutes: 60,
            cors_origins: vec!["*".to_string()],
            oidc: None,
        };
        configure(&mut config);
        config.ensure_artifact_dir().unwrap();

        let pool = db::connect(&config.database_url).await.unwrap();
        db::init_db(&pool).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}:{}", addr.ip(), addr.port());
        config.artifact_base_url = format!("{base_url}/artifacts");
        if let Some(oidc) = config.oidc.as_mut() {
            oidc.redirect_uri = format!("{base_url}/auth/oidc/callback");
        }

        let state = AppState::new(pool.clone(), config.clone()).await.unwrap();
        let app = app_router(state);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
            if let Err(err) = server.await {
                eprintln!("server error: {err}");
            }
        });

        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();

        for attempt in 0..10 {
            let health_url = format!("{}/health", base_url);
            match client.get(&health_url).send().await {
                Ok(response) if response.status().is_success() => break,
                _ if attempt == 9 => panic!("server did not start"),
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }

        Self {
            client,
            base_url,
            pool,
            config,
            shutdown_tx: Some(shutdown_tx),
            _tmpdir: tmp,
        }
    }

    pub fn url(&self, path: &str) -> String {
        if path.starts_with('/') {
            format!("{}{}", self.base_url, path)
        } else {
            format!("{}/{}", self.base_url, path)
        }
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}
