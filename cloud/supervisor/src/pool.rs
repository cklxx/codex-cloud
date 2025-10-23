use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct PoolSettings {
    pub(crate) size: usize,
    pub(crate) template: Option<String>,
    pub(crate) prewarm_hook: Option<LifecycleHook>,
}

#[derive(Clone, Debug)]
pub(crate) struct LifecycleHook {
    command: PathBuf,
}

impl LifecycleHook {
    pub(crate) fn new(command: PathBuf) -> Self {
        Self { command }
    }

    pub(crate) async fn prewarm(&self, template: Option<&str>) -> Result<String> {
        let mut command = Command::new(&self.command);
        command.env("CODEX_SNAPSHOT_EVENT", "prewarm");
        if let Some(template) = template {
            command.env("CODEX_SNAPSHOT_TEMPLATE", template);
        }

        let output = command.output().await.with_context(|| {
            format!("failed to execute prewarm hook {}", self.command.display())
        })?;

        if !output.status.success() {
            return Err(anyhow!(
                "prewarm hook {} exited with status {}: {}",
                self.command.display(),
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let snapshot_id = String::from_utf8(output.stdout)
            .map(|value| value.trim().to_string())
            .with_context(|| "prewarm hook emitted non UTF-8 output")?;

        if snapshot_id.is_empty() {
            return Err(anyhow!(
                "prewarm hook {} must emit a snapshot identifier on stdout",
                self.command.display()
            ));
        }

        Ok(snapshot_id)
    }
}

#[derive(Clone)]
pub(crate) struct SnapshotPool {
    inner: Arc<SnapshotPoolInner>,
}

struct SnapshotPoolInner {
    settings: PoolSettings,
    available: Mutex<VecDeque<String>>,
}

impl SnapshotPool {
    pub(crate) fn new(settings: PoolSettings) -> Self {
        Self {
            inner: Arc::new(SnapshotPoolInner {
                settings,
                available: Mutex::new(VecDeque::new()),
            }),
        }
    }

    pub(crate) async fn ensure_warm_capacity(&self) -> Result<()> {
        let desired = self.inner.settings.size;
        if desired == 0 {
            return Ok(());
        }

        let mut guard = self.inner.available.lock().await;
        while guard.len() < desired {
            drop(guard);
            let snapshot = self.create_snapshot().await?;
            guard = self.inner.available.lock().await;
            guard.push_back(snapshot);
        }
        Ok(())
    }

    pub(crate) async fn checkout(&self) -> Result<SnapshotLease> {
        if self.inner.settings.size == 0 {
            let id = self.create_snapshot().await?;
            return Ok(SnapshotLease {
                id,
                recyclable: false,
            });
        }

        let mut guard = self.inner.available.lock().await;
        if let Some(id) = guard.pop_front() {
            return Ok(SnapshotLease {
                id,
                recyclable: true,
            });
        }
        drop(guard);

        let id = self.create_snapshot().await?;
        Ok(SnapshotLease {
            id,
            recyclable: true,
        })
    }

    pub(crate) async fn recycle(&self, lease: SnapshotLease) -> Result<()> {
        if !lease.recyclable {
            return Ok(());
        }

        let mut guard = self.inner.available.lock().await;
        if guard.len() < self.inner.settings.size {
            guard.push_back(lease.id);
        }
        Ok(())
    }

    pub(crate) async fn discard(&self, _lease: SnapshotLease) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn metrics(&self) -> SnapshotPoolMetrics {
        let guard = self.inner.available.lock().await;
        SnapshotPoolMetrics {
            warm: guard.len(),
            target: self.inner.settings.size,
        }
    }

    async fn create_snapshot(&self) -> Result<String> {
        if let Some(hook) = &self.inner.settings.prewarm_hook {
            return hook.prewarm(self.inner.settings.template.as_deref()).await;
        }

        Ok(format!("snapshot-{}", Uuid::new_v4()))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SnapshotLease {
    id: String,
    recyclable: bool,
}

impl SnapshotLease {
    pub(crate) fn snapshot_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnapshotPoolMetrics {
    pub(crate) warm: usize,
    pub(crate) target: usize,
}
