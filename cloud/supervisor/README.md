# Codex Cloud Supervisor

The supervisor coordinates task execution for Codex Cloud by polling the API
for pending work, materialising prewarmed Firecracker snapshots, and streaming
results back to the control plane.

## Snapshot pool lifecycle

The supervisor maintains a pool of prewarmed executor snapshots. The
`--snapshot-pool-size` flag (or `CODEX_CLOUD_SNAPSHOT_POOL_SIZE` environment
variable) configures the target number of warm snapshots to keep ready. Each
snapshot is produced by invoking the optional prewarm hook defined via
`--prewarm-hook` / `CODEX_CLOUD_PREWARM_HOOK`. The hook receives the
`CODEX_SNAPSHOT_TEMPLATE` environment variable (when provided) and must emit a
snapshot identifier on stdout. The identifier is reused when the snapshot is
recycled back into the pool.

## Repository and dependency caching

Runner instances hydrate a shared cache hierarchy to keep executor start-up
within SLA. By default caches live under `/var/cache/codex`, but the location
can be customised with `--cache-root` / `CODEX_CLOUD_CACHE_ROOT`.

The layout is:

- `${CACHE_ROOT}/git` – bare mirrors keyed by repository UUID.
- `${CACHE_ROOT}/npm` – Node.js package cache for pnpm/npm installs.
- `${CACHE_ROOT}/pip` – Python wheels and virtualenv artifacts.
- `${CACHE_ROOT}/cargo` – Rust crates (cargo registry and git sources).

The supervisor ensures these directories exist before attempts run and records
the absolute paths in execution logs and diffs, making it clear when cache hits
occur. Downstream automation can mount the same paths into executor VMs or
Ignite snapshots to reuse artifacts across runs.
