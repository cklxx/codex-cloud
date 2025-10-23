use std::path::PathBuf;

use tokio::fs;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;

#[derive(Clone)]
pub struct ArtifactStore {
    root: PathBuf,
    base_url: String,
}

impl ArtifactStore {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            root: config.artifacts_dir.clone(),
            base_url: config.artifact_base_url().trim_end_matches('/').to_string(),
        }
    }

    fn path(&self, artifact_id: &str) -> PathBuf {
        self.root.join(artifact_id)
    }

    pub async fn store_text(&self, content: &str, suffix: &str) -> Result<String, AppError> {
        let artifact_id = format!("{}.{}", Uuid::new_v4(), suffix);
        let path = self.path(&artifact_id);
        if let Some(parent) = path.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&path, content).await?;
        Ok(artifact_id)
    }

    pub async fn read_text(&self, artifact_id: &str) -> Result<String, AppError> {
        let path = self.path(artifact_id);
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(content),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(AppError::not_found("Artifact not found"))
            }
            Err(err) => Err(err.into()),
        }
    }

    pub fn artifact_url(&self, artifact_id: &str) -> String {
        format!("{}/{}", self.base_url, artifact_id)
    }
}

pub async fn store_text_artifact(
    store: &ArtifactStore,
    content: &str,
    suffix: &str,
) -> Result<String, AppError> {
    store.store_text(content, suffix).await
}

pub async fn read_artifact(store: &ArtifactStore, artifact_id: &str) -> Result<String, AppError> {
    store.read_text(artifact_id).await
}

pub async fn artifact_url(
    store: &ArtifactStore,
    artifact_id: Option<&str>,
) -> Result<Option<String>, AppError> {
    Ok(artifact_id.map(|id| store.artifact_url(id)))
}
