use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub secret_key: String,
    pub database_url: String,
    pub artifacts_dir: PathBuf,
    pub artifact_base_url: String,
    pub access_token_expire_minutes: u64,
    pub cors_origins: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let secret_key =
            env::var("CODEX_CLOUD_SECRET_KEY").unwrap_or_else(|_| "changeme".to_string());
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://./codex-cloud.db".to_string());
        let artifacts_dir = env::var("CODEX_ARTIFACTS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./artifacts"));
        let artifact_base_url = env::var("CODEX_ARTIFACT_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8000/artifacts".to_string());
        let access_token_expire_minutes = env::var("CODEX_ACCESS_TOKEN_EXPIRE_MINUTES")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(60 * 24);
        let cors_origins = env::var("CODEX_CORS_ORIGINS")
            .unwrap_or_else(|_| "*".to_string())
            .split(',')
            .map(|origin| origin.trim().to_string())
            .filter(|origin| !origin.is_empty())
            .collect::<Vec<_>>();

        Self {
            secret_key,
            database_url,
            artifacts_dir,
            artifact_base_url,
            access_token_expire_minutes,
            cors_origins,
        }
    }

    pub fn artifact_base_url(&self) -> &str {
        &self.artifact_base_url
    }

    pub fn ensure_artifact_dir(&self) -> std::io::Result<()> {
        if !self.artifacts_dir.exists() {
            std::fs::create_dir_all(&self.artifacts_dir)?;
        }
        Ok(())
    }

    pub fn artifact_path(&self, artifact_id: &str) -> PathBuf {
        self.artifacts_dir.join(artifact_id)
    }

    pub fn artifact_url(&self, artifact_id: &str) -> String {
        format!(
            "{}/{}",
            self.artifact_base_url.trim_end_matches('/'),
            artifact_id
        )
    }

    pub fn allow_all_cors(&self) -> bool {
        self.cors_origins.iter().any(|origin| origin == "*")
    }

    pub fn cors_origins(&self) -> Vec<String> {
        self.cors_origins.clone()
    }

    pub fn database_path(&self) -> Option<&Path> {
        if let Some(path) = self.database_url.strip_prefix("sqlite://") {
            Some(Path::new(path))
        } else {
            None
        }
    }
}
