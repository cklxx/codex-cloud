# Codex Cloud - Independent Edition

Codex Cloud is an independently maintained fork of the original Codex developer agent. It keeps the local-first command line experience while providing self-hostable services for task orchestration, artifact storage, and the web dashboard.

The goal of this project is to offer a complete, vendor-neutral stack that can run on personal hardware. All build scripts, documentation, and packaging live in this repository so that you can bootstrap the tooling without relying on upstream releases.

## Table of contents

- [Highlights](#highlights)
- [Installation](#installation)
  - [CLI from source](#cli-from-source)
  - [Docker images and packaged releases](#docker-images-and-packaged-releases)
- [Usage](#usage)
- [Self-hosted Codex Cloud stack](#self-hosted-codex-cloud-stack)
- [Development](#development)
- [Project structure](#project-structure)
- [Community and support](#community-and-support)
- [License](#license)

## Highlights

- Local agent workflow powered by the Rust binaries that ship with the CLI package.
- Task browser and orchestration backend that you can deploy locally or with Docker.
- TypeScript SDK, documented HTTP APIs, and automation hooks for customization.
- Batteries-included workspace that uses pnpm and Cargo with reproducible scripts.

## Installation

Codex Cloud packages are published under the `codex-cloud-*` namespace. Until public packages are available, you can install the CLI directly from source.

### CLI from source

```bash
# Clone this repository
git clone https://github.com/cklxx/codex-cloud.git
cd codex-cloud

# Install JavaScript dependencies
pnpm install

# Build the vendored Rust binary and stage the CLI wrapper
python codex-cli/scripts/install_native_deps.py --platform "$(uname -s | tr '[:upper:]' '[:lower:]')"
```

The CLI entry point is `codex-cli/bin/codex.js`. Symlink it somewhere on your `PATH` to launch Codex Cloud from the terminal:

```bash
ln -s "$(pwd)/codex-cli/bin/codex.js" ~/.local/bin/codex
```

Running `codex` will start the interactive agent with the bundled Rust binary.

### Docker images and packaged releases

This repository publishes releases that include:

- Platform-specific archives of the Rust CLI binary.
- A staged npm tarball for the JavaScript wrapper.
- Docker images for the cloud backend and frontend.

See [docs/release_management.md](./docs/release_management.md) for the end-to-end publishing checklist.

## Usage

Run `codex` to launch the interactive agent. The CLI exposes the same subcommands as the upstream project, with defaults tuned for the independent stack.

To browse Codex Cloud tasks from the terminal, launch the embedded task browser:

```bash
codex --cloud
```

Set the `CODEX_CLOUD_DEFAULT` environment variable to `1` to make the task browser the default entry point. Advanced configuration options are documented in [docs/config.md](./docs/config.md).

## Self-hosted Codex Cloud stack

The `cloud` directory contains the services required to run Codex Cloud end to end. To boot the stack on a single machine:

1. Copy `cloud/.env.example` to `cloud/.env` and fill in secrets.
2. From the repository root, run `make -C cloud dev` (or `docker compose -f cloud/docker-compose.yml up`).
3. Create an admin user with `docker compose -f cloud/docker-compose.yml exec api codex-cloud-backend create-admin <email> <password>`.
4. Point the CLI at `http://localhost:8000` using `CODEX_CLOUD_TASKS_BASE_URL`, or access the web UI at `http://localhost:3000`.

Automated coverage for the task workflow lives in `cloud/backend/tests/task_flow.rs`. Build and test it with `cargo test` from the `cloud/backend` directory.

## Development

Codex Cloud uses pnpm workspaces alongside a Rust workspace. Common commands include:

```bash
# Install workspace dependencies
pnpm install

# Build the Next.js frontend
pnpm --filter codex-cloud-frontend build

# Format documentation and JSON metadata
pnpm run format

# Run backend unit tests
cargo test -p codex-cloud-backend
```

Refer to [PNPM.md](./PNPM.md) for workspace tips and [docs/install.md](./docs/install.md) for system requirements.

## Project structure

- `codex-cli` - JavaScript wrapper that bundles and launches the Rust CLI binary.
- `codex-rs` - Rust workspace that implements the core agent and supporting crates.
- `cloud` - Backend services, background workers, and the Next.js frontend.
- `sdk/typescript` - TypeScript SDK and sample integrations.
- `docs` - Reference documentation for configuration, automation, and deployment.
- `scripts` - Tooling for releases, packaging, and contributor workflows.

## Community and support

Issues and pull requests are welcome. This fork tracks changes independently, so please file discussions in this repository instead of the original upstream tracker.

## License

Codex Cloud - Independent Edition is released under the [Apache-2.0 License](./LICENSE).
