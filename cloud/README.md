# Codex Cloud

## CI Prerequisites

### Secrets
- **`REGISTRY_USERNAME` / `REGISTRY_PASSWORD`** (optional): Required only if publishing the Docker images to an external registry. The default Cloud CI workflow builds images locally without pushing them.
- **`CODECOV_TOKEN`** (optional): Supply if coverage reporting is enabled in future extensions of the pipeline.
- No secrets are needed for the default smoke test; the CLI contract harness bootstraps local credentials against the ephemeral API container.

### Runner Capabilities
- GitHub-hosted `ubuntu-latest` runners are supported. Self-hosted runners must provide:
  - Docker Engine 24+ with the Compose plugin (`docker compose`).
  - Rust toolchain (`rustup`, `cargo`, `rustfmt`) available in `PATH`.
  - Node.js 22+ with `pnpm@10.8.1` for frontend tasks.
  - At least 4 vCPUs and 8 GB RAM to compile Rust crates and execute the Next.js build simultaneously.

### Caching Strategy
- Cargo registry and git index caches are stored via `actions/cache` keyed by the respective crate lockfiles.
- The `cloud/backend` and `cloud/supervisor` targets are cached independently to avoid invalidating unrelated build artifacts.
- `pnpm` dependencies leverage the built-in cache integration from `actions/setup-node`, keyed by `pnpm-lock.yaml`.
- Docker layer caching is not enabled by default; introduce `docker/build-push-action` with registry-backed cache if iterative image builds become slow.

## Local Development
Refer to `cloud/docker-compose.yml` to start the API, frontend, and supervisor locally:

```bash
docker compose -f cloud/docker-compose.yml up --build
```

Run the CLI contract smoke test locally after the stack is healthy:

```bash
ENV_ID=local-dev ./cloud/scripts/cli-contract.sh
```
