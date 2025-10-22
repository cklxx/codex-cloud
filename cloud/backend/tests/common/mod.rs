use std::time::Duration;

use codex_cloud_backend::config::AppConfig;
use codex_cloud_backend::db;
use codex_cloud_backend::routes::app_router;
use codex_cloud_backend::state::AppState;
use reqwest::Client;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub struct TestApp {
    pub client: Client,
    pub base_url: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    _tmpdir: TempDir,
}

impl TestApp {
    pub async fn spawn() -> Self {
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
        };
        config.ensure_artifact_dir().unwrap();

        let pool = db::connect(&config.database_url).await.unwrap();
        db::init_db(&pool).await.unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base_url = format!("http://{}:{}", addr.ip(), addr.port());
        config.artifact_base_url = format!("{base_url}/artifacts");

        let state = AppState::new(pool, config);
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

        Self {
            client,
            base_url,
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
