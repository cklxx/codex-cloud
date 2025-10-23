<p align="center"><code>npm i -g @openai/codex</code><br />or <code>brew install codex</code></p>

<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
</br>
</br>If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE</a>
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a></p>

<p align="center">
  <img src="./.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
  </p>

---

## Quickstart

### Installing and running Codex CLI

Install globally with your preferred package manager. If you use npm:

```shell
npm install -g @openai/codex
```

Alternatively, if you use Homebrew:

```shell
brew install codex
```

Then simply run `codex` to get started:

```shell
codex
```

### Codex Cloud mode

To browse and apply Codex Cloud tasks from the CLI, run the interactive browser:

```shell
codex --cloud
```

Set `CODEX_CLOUD_DEFAULT=1` (or `true`, `yes`, `on`) to make the Codex Cloud task browser the default experience when invoking `codex` without subcommands. See [Codex Cloud architecture](./docs/codex-cloud.md) for details about how the CLI interacts with the hosted service. 要在私有环境中复刻 Codex Cloud，请参考[最小可行版本设计](./docs/codex-cloud-mvp.md)以及更完整的[自建方案蓝图](./docs/codex-cloud-replication-plan.md)。当前路线图明确“先实现 MVP 版本”，相关冲刺节奏与进度更新见 MVP 文档中的[实施启动计划](./docs/codex-cloud-mvp.md#11-实施启动计划)。

### Self-hosted Codex Cloud MVP stack

The repository now includes the first runnable building blocks for the single-machine Codex Cloud MVP described in the docs.

1. Copy `cloud/.env.example` to `cloud/.env` and adjust secrets or database settings as needed.
2. From the repo root run `make -C cloud dev` (or `docker compose -f cloud/docker-compose.yml up`) to build and start the Rust API (`http://localhost:8000`) and the Next.js frontend (`http://localhost:3000`). SQLite data and artifacts persist in the mounted `codex-data` volume.
3. Once the containers are ready, create a bootstrap user with `docker compose -f cloud/docker-compose.yml exec api codex-cloud-backend create-admin <email> <password>` and then sign in via the web UI.
4. Point the CLI at `http://localhost:8000` via `CODEX_CLOUD_TASKS_BASE_URL` or explore the task list, detail, creation, and attempt flows directly in the browser.

Automated coverage for the happy-path workflow lives in `cloud/backend/tests/task_flow.rs`; run it locally with `cargo test` from the `cloud/backend` directory. The frontend builds with `pnpm --filter codex-cloud-frontend build`.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

<p align="center">
  <img src="./.github/codex-cli-login.png" alt="Codex CLI login" width="80%" />
  </p>

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Team, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](./docs/authentication.md#usage-based-billing-alternative-use-an-openai-api-key). If you previously used an API key for usage-based billing, see the [migration steps](./docs/authentication.md#migrating-from-usage-based-billing-api-key). If you're having trouble with login, please comment on [this issue](https://github.com/openai/codex/issues/1243).

### Model Context Protocol (MCP)

Codex can access MCP servers. To configure them, refer to the [config docs](./docs/config.md#mcp_servers).

### Configuration

Codex CLI supports a rich set of configuration options, with preferences stored in `~/.codex/config.toml`. For full configuration options, see [Configuration](./docs/config.md).

---

### Docs & FAQ

- [**Getting started**](./docs/getting-started.md)
  - [CLI usage](./docs/getting-started.md#cli-usage)
  - [Running with a prompt as input](./docs/getting-started.md#running-with-a-prompt-as-input)
  - [Example prompts](./docs/getting-started.md#example-prompts)
  - [Memory with AGENTS.md](./docs/getting-started.md#memory-with-agentsmd)
  - [Configuration](./docs/config.md)
- [**Sandbox & approvals**](./docs/sandbox.md)
- [**Authentication**](./docs/authentication.md)
  - [Auth methods](./docs/authentication.md#forcing-a-specific-auth-method-advanced)
  - [Login on a "Headless" machine](./docs/authentication.md#connecting-on-a-headless-machine)
- **Automating Codex**
  - [GitHub Action](https://github.com/openai/codex-action)
  - [TypeScript SDK](./sdk/typescript/README.md)
  - [Non-interactive mode (`codex exec`)](./docs/exec.md)
- [**Advanced**](./docs/advanced.md)
  - [Tracing / verbose logging](./docs/advanced.md#tracing--verbose-logging)
  - [Model Context Protocol (MCP)](./docs/advanced.md#model-context-protocol-mcp)
- [**Zero data retention (ZDR)**](./docs/zdr.md)
- [**Contributing**](./docs/contributing.md)
- [**Install & build**](./docs/install.md)
  - [System Requirements](./docs/install.md#system-requirements)
  - [DotSlash](./docs/install.md#dotslash)
  - [Build from source](./docs/install.md#build-from-source)
- [**FAQ**](./docs/faq.md)
- [**Open source fund**](./docs/open-source-fund.md)

---

## License

This repository is licensed under the [Apache-2.0 License](LICENSE).
