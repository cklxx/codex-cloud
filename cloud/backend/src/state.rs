use axum::extract::FromRef;
use sqlx::SqlitePool;

use crate::artifacts::ArtifactStore;
use crate::config::AppConfig;
use crate::error::AppError;
use crate::security::OidcProvider;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: AppConfig,
    pub artifacts: ArtifactStore,
    pub oidc: Option<OidcProvider>,
}

impl AppState {
    pub async fn new(pool: SqlitePool, config: AppConfig) -> Result<Self, AppError> {
        let artifacts = ArtifactStore::new(&config);
        let oidc = if let Some(oidc_config) = &config.oidc {
            Some(OidcProvider::discover(oidc_config.clone()).await?)
        } else {
            None
        };

        Ok(Self {
            pool,
            artifacts,
            config,
            oidc,
        })
    }
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for AppConfig {
    fn from_ref(state: &AppState) -> Self {
        state.config.clone()
    }
}

impl FromRef<AppState> for ArtifactStore {
    fn from_ref(state: &AppState) -> Self {
        state.artifacts.clone()
    }
}
