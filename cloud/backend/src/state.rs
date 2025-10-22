use axum::extract::FromRef;
use sqlx::SqlitePool;

use crate::artifacts::ArtifactStore;
use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: AppConfig,
    pub artifacts: ArtifactStore,
}

impl AppState {
    pub fn new(pool: SqlitePool, config: AppConfig) -> Self {
        let artifacts = ArtifactStore::new(&config);
        Self {
            pool,
            artifacts,
            config,
        }
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
