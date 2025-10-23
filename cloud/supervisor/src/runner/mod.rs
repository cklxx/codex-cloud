use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::fs;

use crate::pool::SnapshotLease;
use crate::{AttemptArtifacts, AttemptContext};

#[derive(Clone)]
pub(crate) struct Runner {
    inner: Arc<RunnerInner>,
}

struct RunnerInner {
    cache: CacheLayout,
}

impl Runner {
    pub(crate) async fn new(cache_root: PathBuf) -> Result<Self> {
        let cache = CacheLayout::new(cache_root);
        cache.ensure_directories().await?;
        Ok(Self {
            inner: Arc::new(RunnerInner { cache }),
        })
    }

    pub(crate) async fn execute(
        &self,
        context: &AttemptContext,
        snapshot: &SnapshotLease,
    ) -> Result<AttemptArtifacts> {
        let repository_cache = self.inner.cache.prepare_repository_cache(context).await?;

        let timestamp = Utc::now().to_rfc3339();
        let diff = build_diff(
            context,
            &timestamp,
            snapshot,
            &self.inner.cache,
            repository_cache.as_deref(),
        );
        let log = build_log(
            context,
            &timestamp,
            snapshot,
            &self.inner.cache,
            repository_cache.as_deref(),
        );

        Ok(AttemptArtifacts {
            diff: Some(diff),
            log: Some(log),
        })
    }
}

#[derive(Debug, Clone)]
struct CacheLayout {
    root: PathBuf,
    git: PathBuf,
    npm: PathBuf,
    pip: PathBuf,
    cargo: PathBuf,
}

impl CacheLayout {
    fn new(root: PathBuf) -> Self {
        let git = root.join("git");
        let npm = root.join("npm");
        let pip = root.join("pip");
        let cargo = root.join("cargo");
        Self {
            root,
            git,
            npm,
            pip,
            cargo,
        }
    }

    async fn ensure_directories(&self) -> Result<()> {
        for path in [&self.root, &self.git, &self.npm, &self.pip, &self.cargo] {
            fs::create_dir_all(path)
                .await
                .with_context(|| format!("failed to create cache directory {}", path.display()))?;
        }
        Ok(())
    }

    async fn prepare_repository_cache(&self, context: &AttemptContext) -> Result<Option<PathBuf>> {
        let Some(detail) = context.detail.as_ref() else {
            return Ok(None);
        };
        let Some(repository) = detail.repository.as_ref() else {
            return Ok(None);
        };

        let mirror_path = self.git.join(repository.id.to_string());
        fs::create_dir_all(&mirror_path).await.with_context(|| {
            format!("failed to prepare git mirror at {}", mirror_path.display())
        })?;
        Ok(Some(mirror_path))
    }
}

fn build_diff(
    context: &AttemptContext,
    timestamp: &str,
    snapshot: &SnapshotLease,
    cache: &CacheLayout,
    repository_cache: Option<&Path>,
) -> String {
    let mut diff = String::new();
    diff.push_str("diff --git a/TASK_LOG.md b/TASK_LOG.md\n");
    diff.push_str("--- a/TASK_LOG.md\n");
    diff.push_str("+++ b/TASK_LOG.md\n");
    diff.push_str("@@\n");
    diff.push_str(&format!(
        "+## Task {} ({})\\n",
        context.task.id, context.task.title
    ));
    diff.push_str(&format!(
        "+Processed at {timestamp} UTC by codex-cloud-supervisor\\n"
    ));
    diff.push_str(&format!("+Using snapshot: {}\\n", snapshot.snapshot_id()));
    diff.push_str(&format!("+Cache root: {}\\n", cache.root.display()));
    if let Some(repository_cache) = repository_cache {
        diff.push_str(&format!(
            "+Repository mirror cache: {}\\n",
            repository_cache.display()
        ));
    }
    diff.push_str(&format!("+npm cache: {}\\n", cache.npm.display()));
    diff.push_str(&format!("+pip cache: {}\\n", cache.pip.display()));
    diff.push_str(&format!("+cargo cache: {}\\n", cache.cargo.display()));

    if let Some(detail) = context.detail.as_ref() {
        diff.push_str(&format!("+Detail ID: {}\\n", detail.id));
        diff.push_str(&format!("+Snapshot title: {}\\n", detail.title));
        if let Some(environment_id) = &detail.environment_id {
            diff.push_str(&format!("+Environment: {}\\n", environment_id));
        }
        if let Some(repository) = &detail.repository {
            diff.push_str(&format!(
                "+Repository: {} ({}) on branch {} (id {})\\n",
                repository.name, repository.git_url, repository.default_branch, repository.id
            ));
        }
        if let Some(description) = &detail.description {
            for line in description.lines() {
                diff.push_str("+> ");
                diff.push_str(line);
                diff.push('\n');
            }
        }
    }

    diff
}

fn build_log(
    context: &AttemptContext,
    timestamp: &str,
    snapshot: &SnapshotLease,
    cache: &CacheLayout,
    repository_cache: Option<&Path>,
) -> String {
    let mut log = format!(
        "[{timestamp}] Attempt {} succeeded for task {} ({})",
        context.attempt.id, context.task.id, context.task.title
    );

    log.push_str(&format!(
        "\nUsing prewarmed snapshot: {}",
        snapshot.snapshot_id()
    ));
    log.push_str("\nCache hits:");
    if let Some(repository_cache) = repository_cache {
        log.push_str(&format!(
            "\n- Git mirror: {} (hit)",
            repository_cache.display()
        ));
    } else {
        log.push_str("\n- Git mirror: miss");
    }
    log.push_str(&format!("\n- npm cache: {}", cache.npm.display()));
    log.push_str(&format!("\n- pip cache: {}", cache.pip.display()));
    log.push_str(&format!("\n- cargo cache: {}", cache.cargo.display()));

    if let Some(detail) = &context.detail {
        if let Some(repository) = &detail.repository {
            log.push_str(&format!(
                "\nRepository: {} ({}) default branch {} (id {})",
                repository.name, repository.git_url, repository.default_branch, repository.id
            ));
        }
        if let Some(environment_id) = &detail.environment_id {
            log.push_str(&format!("\nEnvironment: {}", environment_id));
        }
        if let Some(description) = &detail.description {
            log.push_str("\nTask description:\n");
            log.push_str(description);
        }
    }

    log
}
