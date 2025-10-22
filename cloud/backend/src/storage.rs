use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use http::Method;
use minio::s3::client::Client;
use minio::s3::creds::StaticProvider;
use minio::s3::error::{Error as MinioError, ErrorCode};
use minio::s3::http::BaseUrl;
use minio::s3::types::S3Api;
use tokio::fs;
use uuid::Uuid;

use crate::config::{AppConfig, S3Config};
use crate::error::AppError;

#[derive(Clone)]
pub struct ArtifactStore {
    inner: Arc<ArtifactStoreInner>,
}

#[derive(Clone)]
enum ArtifactStoreInner {
    Local(LocalStore),
    S3(S3Store),
}

#[derive(Clone)]
struct LocalStore {
    root: PathBuf,
    base_url: String,
    prefix: Option<String>,
}

#[derive(Clone)]
struct S3Store {
    client: Client,
    bucket: String,
    prefix: Option<String>,
    presign_ttl_seconds: u64,
}

impl ArtifactStore {
    pub fn from_config(config: &AppConfig) -> Result<Self, AppError> {
        let inner = if let Some(s3) = config.s3_config() {
            ArtifactStoreInner::S3(S3Store::new(
                s3,
                config.artifact_prefix(),
                config.artifact_url_ttl_seconds(),
            )?)
        } else {
            ArtifactStoreInner::Local(LocalStore::new(
                config.artifacts_dir.clone(),
                config.artifact_base_url().to_string(),
                config.artifact_prefix().map(|value| value.to_string()),
            ))
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    pub async fn store_text(&self, content: &str, suffix: &str) -> Result<String, AppError> {
        match self.inner.as_ref() {
            ArtifactStoreInner::Local(store) => store.store_text(content, suffix).await,
            ArtifactStoreInner::S3(store) => store.store_text(content, suffix).await,
        }
    }

    pub async fn read_text(&self, artifact_id: &str) -> Result<String, AppError> {
        match self.inner.as_ref() {
            ArtifactStoreInner::Local(store) => store.read_text(artifact_id).await,
            ArtifactStoreInner::S3(store) => store.read_text(artifact_id).await,
        }
    }

    pub async fn artifact_url(&self, artifact_id: &str) -> Result<String, AppError> {
        match self.inner.as_ref() {
            ArtifactStoreInner::Local(store) => Ok(store.artifact_url(artifact_id)),
            ArtifactStoreInner::S3(store) => store.presign_url(artifact_id).await,
        }
    }
}

impl LocalStore {
    fn new(root: PathBuf, base_url: String, prefix: Option<String>) -> Self {
        Self {
            root,
            base_url,
            prefix,
        }
    }

    fn path(&self, artifact_id: &str) -> PathBuf {
        match &self.prefix {
            Some(prefix) => self.root.join(prefix).join(artifact_id),
            None => self.root.join(artifact_id),
        }
    }

    fn url_prefix(&self) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}/{prefix}", self.base_url.trim_end_matches('/')),
            None => self.base_url.trim_end_matches('/').to_string(),
        }
    }

    async fn store_text(&self, content: &str, suffix: &str) -> Result<String, AppError> {
        let artifact_id = format!("{}.{}", Uuid::new_v4(), suffix);
        let path = self.path(&artifact_id);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        fs::write(&path, content).await?;
        Ok(artifact_id)
    }

    async fn read_text(&self, artifact_id: &str) -> Result<String, AppError> {
        let path = self.path(artifact_id);
        match fs::read_to_string(&path).await {
            Ok(content) => Ok(content),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(AppError::not_found("Artifact not found"))
            }
            Err(err) => Err(err.into()),
        }
    }

    fn artifact_url(&self, artifact_id: &str) -> String {
        format!("{}/{}", self.url_prefix(), artifact_id)
    }
}

impl S3Store {
    fn new(
        config: &S3Config,
        prefix: Option<&str>,
        presign_ttl_seconds: u64,
    ) -> Result<Self, AppError> {
        let mut base_url = BaseUrl::from_str(&config.endpoint)
            .map_err(|err| AppError::storage(err.to_string()))?;
        base_url.region = config.region.clone();
        if config.use_path_style {
            base_url.virtual_style = false;
        }

        let provider = StaticProvider::new(&config.access_key, &config.secret_key, None);
        let client = Client::new(base_url, Some(Box::new(provider)), None, None)
            .map_err(|err| AppError::storage(err.to_string()))?;

        Ok(Self {
            client,
            bucket: config.bucket.clone(),
            prefix: prefix.map(|value| value.trim_matches('/').to_string()),
            presign_ttl_seconds,
        })
    }

    fn key(&self, artifact_id: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{prefix}/{}", artifact_id),
            None => artifact_id.to_string(),
        }
    }

    async fn store_text(&self, content: &str, suffix: &str) -> Result<String, AppError> {
        let artifact_id = format!("{}.{}", Uuid::new_v4(), suffix);
        let key = self.key(&artifact_id);
        self.client
            .put_object_content(self.bucket.clone(), key, content.to_owned())
            .content_type("text/plain".to_string())
            .send()
            .await
            .map_err(|err| AppError::storage(err.to_string()))?;
        Ok(artifact_id)
    }

    async fn read_text(&self, artifact_id: &str) -> Result<String, AppError> {
        let key = self.key(artifact_id);
        let response = self
            .client
            .get_object(self.bucket.clone(), key)
            .send()
            .await
            .map_err(Self::map_read_error)?;
        let segmented = response
            .content
            .to_segmented_bytes()
            .await
            .map_err(|err| AppError::storage(err.to_string()))?;
        String::from_utf8(segmented.to_bytes().to_vec())
            .map_err(|err| AppError::storage(err.to_string()))
    }

    async fn presign_url(&self, artifact_id: &str) -> Result<String, AppError> {
        let key = self.key(artifact_id);
        let expiry: u32 = self
            .presign_ttl_seconds
            .try_into()
            .map_err(|_| AppError::storage("Artifact URL TTL exceeds supported range"))?;
        let response = self
            .client
            .get_presigned_object_url(self.bucket.clone(), key, Method::GET)
            .expiry_seconds(expiry)
            .send()
            .await
            .map_err(|err| AppError::storage(err.to_string()))?;
        Ok(response.url)
    }

    fn map_read_error(err: MinioError) -> AppError {
        match err {
            MinioError::S3Error(response)
                if matches!(
                    response.code,
                    ErrorCode::NoSuchKey | ErrorCode::ResourceNotFound
                ) =>
            {
                AppError::not_found("Artifact not found")
            }
            other => AppError::storage(other.to_string()),
        }
    }
}
