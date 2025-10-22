# Codex Cloud architecture

Codex Cloud is the hosted experience for browsing, launching, and applying agent tasks from OpenAI's infrastructure. The CLI embeds a dedicated task browser that interacts with the same backend APIs exposed in ChatGPT's Codex product. This document summarizes how the open-source CLI integrates with that service and how to enable the workflow locally.

## Entry points in the CLI

The native CLI exposes Codex Cloud in two complementary ways:

- `codex cloud` (or the alias `codex cloud-tasks`) launches the full-screen task browser implemented in the `codex-cloud-tasks` crate. `codex_cloud_tasks::run_main` wires the terminal UI to the backend client while respecting shared configuration flags.
- Passing `--cloud` when running `codex` switches the default experience from the local interactive agent to the Codex Cloud browser. The flag participates in Clap parsing on the root CLI, so it composes with other global overrides.
- Setting `CODEX_CLOUD_DEFAULT` to a truthy value (`1`, `true`, `yes`, or `on`) opts into Codex Cloud without passing `--cloud`. The launcher checks the environment before deciding which interface to start.

## Backend client lifecycle

The task browser builds a backend client through `init_backend`, which selects between a mock implementation and the production HTTP client. The default base URL targets `https://chatgpt.com/backend-api`, but it can be overridden via `CODEX_CLOUD_TASKS_BASE_URL`. Authentication piggybacks on the ChatGPT login stored in Codex's config directory, and the client adds the `ChatGPT-Account-Id` header when available.

When tasks are loaded, the browser requests them from the backend with a five-second timeout and filters out review-only entries so that the main queue shows actionable work.

## Submitting new tasks programmatically

The `codex cloud exec` subcommand provides a non-interactive path for creating tasks. It accepts a prompt, selects a target environment by ID or label, and optionally requests multiple attempts. The resolver normalizes environment identifiers by inspecting the Codex Cloud environments list and supports human-friendly labels as fallbacks.

## Local defaults and configuration

Users who prefer the hosted workflow can set `CODEX_CLOUD_DEFAULT` to make Codex Cloud the default when launching `codex`. The same environment variable is respected by scripts and terminals alike. Documentation in the main README links back to this guide for a full overview.

## 自建 Codex Cloud 的起点

开源 CLI 不附带托管后端或 Web 页面。如果需要在私有环境中重现 Codex Cloud，可以先按照《[Codex Cloud 最小可行版本设计](./codex-cloud-mvp.md)》交付单机 Docker Compose + Firecracker 的 MVP，再参考《[Codex Cloud 自建方案设计](./codex-cloud-replication-plan.md)》扩展企业级能力。

> **实施顺序提醒**：团队目前正在执行 MVP 落地冲刺，节奏安排、任务划分与状态跟踪位于《[Codex Cloud 最小可行版本设计](./codex-cloud-mvp.md#11-实施启动计划)》。所有增强型需求需等待该节列出的里程碑完成后再排期。

> 🔨 最新进展：`cloud/backend` 目录已经落地 Rust (Axum) 单体与端到端测试，`cloud/frontend` 交付了基于 Next.js + Ant Design 的任务浏览 UI。通过 `cloud/docker-compose.yml` 可在单机启动 API（8000 端口）与前端（3000 端口），后续将继续集成执行器与对象存储能力。
