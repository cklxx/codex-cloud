# Codex Cloud 最小可行版本（MVP）设计（单机部署版）

> 目标：在单台服务器上复刻 Codex Cloud 的核心体验，优先交付一个前后端贯通、可执行任务的最小可行产品。方案聚焦“开箱即跑”，使用 Docker Compose 一键启动所有组件，并针对执行容器的启动速度提供经过调研的最优实践。

## 1. MVP 范围

- **终端用户能力**
  - 浏览任务列表与基本筛选（状态、仓库）。
  - 查看任务详情、diff、最新尝试日志。
  - 领取任务、触发一次执行尝试、提交结果。
  - 通过 Web 前端触发最简单的任务创建（自由文本 + 仓库选择）。
- **工程能力**
  - 单租户登录（内置本地账户或对接公司 OAuth，任选其一）。
  - 后端 API 与 CLI `CloudBackend` 协议兼容的最小子集（列出任务、获取详情、创建尝试、提交结果）。
  - **单机 Firecracker 微虚拟机执行器**：通过 Ignite 管理的 Firecracker microVM 在 <300 ms 内就绪，支持单次执行并收集 diff。
  - 所有组件（Web、API、数据库、对象存储、执行调度器）通过 Docker Compose 在单台服务器启动。
  - 任务状态机：`Pending → Claimed → Running → Review → Applied`。
- **非目标（MVP 后续迭代）**
  - 多租户隔离、RBAC 细粒度权限。
  - 并行尝试/最佳结果评分、审批流、Webhook。
  - 完整的审计、可观测性体系（仅保留基础日志）。

## 2. 架构快照

```
┌──────────────────────────────┐
│         Web 前端 (SPA)       │ ← Next.js 容器运行 `next start`（后续可接入 Caddy/HTTPS）
└──────────────┬───────────────┘
               │REST JSON
┌──────────────▼───────────────┐
│   Codex API 单体服务          │ ← Rust (Axum) 单体，负责鉴权与业务逻辑
├──────────────┬───────────────┤
│ Task/Attempt │ Artifact/File │ ← sqlx + async worker，持久化到 SQLite + 本地文件系统
└──────────────┴───────────────┘
               │Unix socket RPC
┌──────────────▼───────────────┐
│ Firecracker Executor Supervisor│ ← 常驻进程，使用 Ignite 启动 microVM
└──────────────┬───────────────┘
               │
        ┌──────▼────────┬──────────┐
        │ SQLite (文件) │ 本地 Artifact │ ← 通过 docker volume 挂载到同一主机
        └──────────────┴──────────┘

（全部组件通过 `docker compose up` 在单机启动，执行器与 API 使用本地网络互通。）
```

- 前端与 API 运行在同一台物理服务器的容器中，通过 Docker 网络通信。
- 执行器无需外部消息队列，直接轮询 SQLite `task_attempts` 表获取待执行记录（后续迭代可平滑迁移至 PostgreSQL）。
- CLI 通过配置 `CODEX_CLOUD_TASKS_BASE_URL` 指向本地服务器，体验与 Web 端共用同一后端。

## 3. Web 前端蓝图

| 页面     | 关键组件                                 | 数据来源                              | MVP 功能点                                      |
| -------- | ---------------------------------------- | ------------------------------------- | ----------------------------------------------- |
| 登录页   | 本地账户表单（Ant Design Form）          | API `/auth/session`                   | 支持本地账户登录、注册、错误反馈                |
| 任务列表 | 表格、状态标签、搜索框                   | `/tasks`                              | 展示任务标题、状态、更新时间；支持状态筛选      |
| 任务详情 | 任务描述、尝试历史、文本 diff/log 查看器 | `/tasks/{id}`、`/tasks/{id}/attempts` | 展示 diff/log 文本、领取/执行按钮、尝试完成表单 |
| 任务创建 | 表单（标题、描述、仓库、环境标签）       | `/tasks` (POST)                       | 触发后端创建任务并排队                          |

- UI 组件库：Chakra UI / Ant Design（二选一）以加快交付，当前以 `next build` + `next start` 提供运行时；后续可按需切换为静态导出。
- 静态资源由 Caddy 容器提供 HTTPS，证书由 Caddy 自动获取（或导入自签名证书）。
- 当前版本使用 Ant Design + `react-diff-view` + `react-virtuoso` 展示 diff/log，提供语法感知 diff 与虚拟滚动体验。

## 4. 后端 API 合约（MVP 子集）

| 功能     | 方法                           | 请求示例                               | 响应                                             | CLI 兼容点                                         |
| -------- | ------------------------------ | -------------------------------------- | ------------------------------------------------ | -------------------------------------------------- |
| 登录会话 | `POST /auth/session`           | OAuth 回调码或用户名/密码              | `{ "token": "..." }`                             | CLI 可注入 Bearer Token 到 `CODEX_CLOUD_API_TOKEN` |
| 列出任务 | `GET /tasks?status=ready`      | Header: `Authorization`                | `[{ id, title, status, repo, updated_at }]`      | 对应 `CloudBackend::list_tasks`                    |
| 获取详情 | `GET /tasks/{id}`              |                                        | `{ id, description, diff_url, current_attempt }` | 对应 `get_task_text` / `get_task_diff`             |
| 创建任务 | `POST /tasks`                  | `{ title, description, repo_id, env }` | `{ id }`                                         | 对应 `create_task`                                 |
| 领取任务 | `POST /tasks/{id}/claim`       |                                        | `{ claim_expires_at }`                           | 对应 `claim_task`                                  |
| 提交尝试 | `POST /tasks/{id}/attempts`    | `{ environment_id }`                   | `{ attempt_id }`                                 | CLI `apply_task` 时触发执行                        |
| 上报结果 | `POST /attempts/{id}/complete` | `{ status, diff, log_url }`            | `{}`                                             | CLI 轮询 `list_sibling_attempts` 获取              |

- 所有 diff / 日志存储到对象存储（MinIO 本地实例），返回预签名 URL。
- MVP 阶段不提供 WebSocket；执行器推送完成后，前端轮询任务详情。

## 5. 执行器设计（Firecracker 单机版）

1. **触发机制**：

- Supervisor 进程每 2 秒轮询 SQLite `task_attempts`，挑选 `status = "queued"` 的记录。
- 使用 Ignite CLI (`ignite run`) 启动 Firecracker microVM，选择预热好的 snapshot。

2. **执行步骤**：

   - 通过 snapshot 恢复 microVM，启动脚本挂载只读基础镜像和可写 overlay。
   - 在 microVM 内克隆仓库快照（使用本地 bare 仓库缓存）。
   - 安装任务依赖（若 snapshot 已预装常用语言环境，仅拉取缺失依赖）。
   - 运行模型代理脚本生成代码变更，并将 diff/log 上传到 MinIO。
   - 调用 API `complete` 上传结果，随后销毁 microVM。

3. **镜像与快照策略**：

   - 基础镜像：Debian slim + git + pnpm + Rust toolchain + Python。构建后通过 Ignite 生成 Firecracker snapshot。
   - 启动脚本在 snapshot 中预载常用依赖缓存（npm、pip、cargo registry），缩短冷启动时间。
   - Supervisor 维护一个 3~5 个微虚拟机的预热池，避免高并发时出现冷启动尖峰。

4. **安全隔离**：
   - Firecracker microVM 提供内核级隔离，限制网络访问（默认出网关闭，仅允许拉取依赖的特定域名）。
   - API Token 通过只读卷注入，执行完成后自动销毁。

## 6. 容器启动速度调研与选型

| 方案                               | 平均启动时延                                  | 优势                         | 劣势                          | 结论                       |
| ---------------------------------- | --------------------------------------------- | ---------------------------- | ----------------------------- | -------------------------- |
| Docker / containerd 标准容器       | 0.8~1.2 秒（空镜像）                          | 生态成熟、易用               | 启动路径长，冷镜像拉取慢      | 作为基线，仅在开发模式使用 |
| Kata Containers                    | ~0.4 秒（带 VM 隔离）                         | 强隔离、兼容 OCI             | 需要额外内核模块              | 可作为后续多租户演进方向   |
| **Firecracker（Ignite snapshot）** | **120~250 ms**（官方性能报告，snapshot 预热） | 微秒级开销、强隔离、启动最快 | 需要 KVM 支持，镜像需要预处理 | **MVP 首选**               |

- Firecracker 团队披露的基准数据显示，通过 snapshot 恢复可在 125 ms 内完成 microVM 启动，即使加上初始化脚本也能稳定控制在 300 ms 内。[^firecracker]
- Ignite 提供容器式 UX（`ignite run`），可直接在单机 Docker Compose 中运行，并允许以 OCI 镜像为基础制作 snapshot。[^ignite]
- 为进一步降低延迟，可结合 [stargz-snapshotter](https://github.com/containerd/stargz-snapshotter) 或 lazy-pull 技术对基础镜像进行按需加载，避免首次启动拉取大镜像。

[^firecracker]: Firecracker "Performance" 文档指出恢复 snapshot 的启动时延约 125 ms，详见 <https://github.com/firecracker-microvm/firecracker/blob/main/docs/performance.md>。

[^ignite]: Weaveworks Ignite 将 OCI 镜像转换为 Firecracker microVM，并在官方文档中强调子秒级启动，详见 <https://ignite.readthedocs.io/en/stable/overview.html>。

## 7. 数据模型概览

```sql
-- 用户
CREATE TABLE users (
  id UUID PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  name TEXT,
  auth_provider TEXT NOT NULL,
  created_at TIMESTAMP DEFAULT now()
);

-- 仓库
CREATE TABLE repositories (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL,
  git_url TEXT NOT NULL,
  default_branch TEXT NOT NULL
);

-- 任务
CREATE TABLE tasks (
  id UUID PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT,
  repository_id UUID REFERENCES repositories(id),
  status TEXT CHECK (status IN ('pending','claimed','running','review','applied')),
  assignee_id UUID REFERENCES users(id),
  created_by UUID REFERENCES users(id),
  created_at TIMESTAMP DEFAULT now(),
  updated_at TIMESTAMP DEFAULT now()
);

-- 尝试
CREATE TABLE task_attempts (
  id UUID PRIMARY KEY,
  task_id UUID REFERENCES tasks(id),
  status TEXT CHECK (status IN ('queued','running','succeeded','failed')),
  diff_url TEXT,
  log_url TEXT,
  score NUMERIC,
  created_at TIMESTAMP DEFAULT now(),
  updated_at TIMESTAMP DEFAULT now()
);
```

## 8. 部署建议（单机版）

- **环境要求**：
  - Linux 主机（Ubuntu 22.04+），启用 KVM（`/dev/kvm` 可访问）。
  - 安装 Docker、Docker Compose v2、Ignite (`curl -sfL https://ignite.run/install.sh | sh`)。
- **Compose 拓扑**：
  - `frontend`：Next.js 应用容器运行 `next start` 并监听 3000 端口。
- `api`：Rust (Axum) 单体，内嵌 sqlx + SQLite。
  - `postgres`：持久化卷 `/var/lib/codex/postgres`。
  - `minio`：对象存储，开启浏览器端管理页面方便调试。
  - `executor`：以 host 网络运行的 Supervisor，挂载 `/var/lib/firecracker`。
- **部署步骤**：
  1. `git clone` 项目并拷贝 `.env.example`；填写 OAuth 配置或创建本地管理员账户。
  2. `docker compose build && docker compose up -d`。
  3. 执行 `ignite snapshot create codex-runner` 生成执行器 snapshot；在 `.env` 中写入 snapshot 名称。
  4. 打开 `https://<host>/` 访问 Web；CLI 通过 `CODEX_CLOUD_TASKS_BASE_URL=https://<host>/api` 连接。
- **运维要点**：
  - 通过 systemd unit 保证 Docker Compose 与 Ignite Supervisor 随机器启动。
- 备份 SQLite 数据文件与 artifact 目录；后续切换对象存储时再接入版本化策略。
  - 监控重点：API p95 延迟、Firecracker 启动失败率、任务执行成功率。

## 9. 里程碑拆解

| 里程碑        | 预期交付                                               | 工期（理想人力 3~4 人） |
| ------------- | ------------------------------------------------------ | ----------------------- |
| M0 准备       | 需求冻结、技术栈选型、搭建开发环境                     | 1 周                    |
| M1 后端骨架   | Rust (Axum) 单体 + SQLite + 基础 API                   | 2 周                    |
| M2 前端初版   | Next.js + Ant Design，完成列表/详情/创建               | 2 周                    |
| M3 执行器接入 | Firecracker snapshot、Ignite Supervisor、diff/日志回传 | 3 周                    |
| M4 CLI 验证   | CLI 指向单机 API，完成端到端任务执行                   | 1 周                    |

## 10. 实施 TODO 列表（分模块拆解）

> 勾选顺序建议遵循里程碑，但团队可视资源并行推进。所有任务默认落地在单机 Compose 环境，并要求提交 IaC/脚本以便复现。

### 10.1 平台基础

- [ ] 完成系统设计评审与安全评估备案（M0 输出）。
- [x] 编写 `.env.example` 与环境变量文档，覆盖 OAuth、本地账户、存储路径、Ignite snapshot 名称。（已新增 `cloud/.env.example`）
- [x] 拉起开发用 Docker Compose（含 Hot reload）并提供 `make dev`/`just dev` 入口。（`cloud/docker-compose.yml` + `Makefile dev`）
- [ ] 构建 CI pipeline：Lint（前端/后端）、单元测试、集成测试、镜像构建、Compose 烟囱部署。

### 10.2 后端 API

- [x] Scaffold Rust (Axum) 工程，接入 sqlx + SQLite 的最小持久层。（`cloud/backend`）
- [x] 实现鉴权：本地账户（bcrypt + JWT）已落地，OAuth（OpenID Connect）待配置。
- [x] 落地 MVP API（任务 CRUD、claim、attempt、complete），提供 JSON Schema/端到端测试。（Axum + sqlx）
- [x] 接入对象存储（MinIO SDK），实现 diff/log 上传与预签名 URL 返回。（`cloud/backend/src/storage.rs`、`cloud/backend/src/artifacts.rs`）
- [x] 编写 API 层测试（Rust async 集成测试），覆盖核心 happy path 与鉴权。
- [x] 提供 CLI 兼容性契约测试脚本（使用现有 `codex cloud exec` 流程，见 `cloud/scripts/cli-contract.sh`）。

### 10.3 Web 前端

- [x] 使用 Next.js + Ant Design 搭建 SPA；深色模式由 Ant Design `darkAlgorithm` 提供，国际化暂保留中文默认配置。
- [x] 实现登录、任务列表、任务详情、任务创建页面，并接入 API。
- [x] 集成 diff/log 组件（react-diff-view + react-virtuoso）。
- [x] 打通任务领取与执行触发流程（调用 `POST /tasks/{id}/attempts`）。
- [x] 增加全局错误处理与请求超时反馈（Ant Design message 提供交互提示，后续补充请求超时监控）。
- [x] 构建脚本通过 `cloud/frontend/Dockerfile` 输出 Next.js 运行镜像；后续如需静态托管再落盘至 `frontend/out`。

### 10.4 执行器与 Firecracker

- [x] 编写 Supervisor（Rust/Go/Python 任一），实现数据库轮询、任务出队与状态回写。（新增 `cloud/supervisor` Rust 服务）
- [ ] 制作基础 OCI 镜像并使用 Ignite 生成 Firecracker snapshot；记录构建脚本。
- [ ] 实现 snapshot 预热池（预热池策略待实现）。
- [x] 提供可配置的最大并发控制（Supervisor 通过并发上限配置串行/并行执行）。
- [ ] 集成仓库缓存与依赖预热脚本，保证 <300 ms 启动目标。
- [x] 对接 MinIO 上传日志、diff，并调用 API `complete`。（Supervisor 通过 `/tasks/attempts/{id}/complete` 上报文本工件，由后端写入对象存储。）
- [x] 编写集成测试：模拟任务入队、触发执行、校验 diff/log 上传与状态流转。（`cloud/supervisor` 使用 WireMock 覆盖完整提交流程。）

### 10.5 运维与观测

- [ ] 完成 Docker Compose `production.yml`，包括持久化卷、资源限制、健康检查。
- [ ] 编写 systemd unit 与开机自启脚本（Docker Compose + Ignite Supervisor）。
- [ ] 接入基础监控：Prometheus node exporter、Firecracker 指标、API p95 监控（可使用 Grafana 套件）。
- [ ] 提供备份/恢复脚本：SQLite 文件快照、本地 artifact 目录打包。
- [ ] 整理运维手册与应急预案（常见故障、日志定位、snapshot 重新生成流程）。

## 11. 实施启动计划

> 本节将里程碑拆解为具体冲刺与日程，确保团队“先实现 MVP 版本”的指令得到落实，并形成可跟踪的落地节奏。

### 11.1 当前阶段目标

- **阶段定位**：M1~M3 冲刺正式启动，目标是在 6 周内完成“后端骨架 + 前端初版 + 执行器接入”三大里程碑。
- **团队编组**：
  - 平台/运维 1 人（负责 Compose、CI/CD、基础设施脚本）。
  - 后端 2 人（API、数据库、对象存储、CLI 契约测试）。
  - 前端 1 人（Next.js、UI、E2E 测试）。
  - 执行器 1 人（Firecracker snapshot、Supervisor、性能验证）。
- **共识**：所有功能在单机 Docker Compose 环境交付；非 MVP 范围的需求统一记录在 backlog，不影响当前冲刺。

### 11.2 前两周冲刺计划（T0~T14）

| 日期区间 | Owner  | 主要输出                                        | 对应 TODO              | 状态                                      |
| -------- | ------ | ----------------------------------------------- | ---------------------- | ----------------------------------------- |
| T0~T2    | 平台   | `.env.example`、开发用 Compose、`make dev` 脚本 | 平台基础第 2、3 项     | ✅ 已完成                                 |
| T0~T4    | 后端 A | Rust (Axum) Scaffold、sqlx 迁移脚本             | 后端 API 第 1 项       | ✅ 已完成                                 |
| T0~T5    | 后端 B | 鉴权（本地账户 + OAuth）与 Token 校验           | 后端 API 第 2 项       | 🟡 进行中（本地账户已上线，OAuth 待评审） |
| T3~T7    | 前端   | Next.js + Ant Design 项目初始化、登录页         | 前端第 1、2 项（部分） | ✅ 已完成                                 |
| T5~T10   | 执行器 | 基础 OCI 镜像、Ignite snapshot、预热脚本草案    | 执行器第 2、4 项       | 未开始                                    |
| T8~T14   | 全体   | 端到端冒烟：API + 前端登录 + CLI list           | TODO 各模块首项        | 未开始                                    |

> “状态”栏用于每日站会更新，可用 ✅/🟡/🟥 替换文字。完成后需同步检查对应里程碑打勾。

### 11.3 里程碑追踪面板

| 里程碑        | 负责人       | 目标完成日 | 当前状态                                  | 阻塞项                                |
| ------------- | ------------ | ---------- | ----------------------------------------- | ------------------------------------- |
| M0 准备       | 平台 Owner   | T0         | ✅ 已完成                                 | -                                     |
| M1 后端骨架   | 后端 A/B     | T14        | 🟡 进行中（Scaffold 已提交）              | OAuth 配置待安全评审                  |
| M2 前端初版   | 前端 Owner   | T21        | ✅ 已完成（核心页面 + diff/log 组件上线） | -                                     |
| M3 执行器接入 | 执行器 Owner | T35        | 🟥 未启动                                 | snapshot 基础镜像等待平台提供构建节点 |
| M4 CLI 验证   | 全体         | T42        | 🟥 未启动                                 | 依赖 M1~M3 完成                       |

### 11.4 日常仪式与交付要求

- **每日站会**：汇报昨日进展、当日计划、阻塞项，更新上表状态。
- **周度评审**：每周五演示前端 + API + 执行器当前成果，确认是否满足对应里程碑入口条件。
- **验收清单**：每个里程碑完成时，需勾选本节对应 TODO，并提前准备在第 12 节描述的验证步骤。
- **文档同步**：任何偏离 MVP 范围的讨论与需求统一追加到《后续拓展方向》或 `docs/codex-cloud-replication-plan.md` 的 backlog 中，避免侵占当前冲刺。

## 12. 验证方案

> 验证目标：确保单机部署在功能、性能、安全三方面满足 MVP 定义，并可复制部署。

### 11.1 自动化测试矩阵

| 类别                | 覆盖范围                                 | 触发方式                                            | 通过判定                                       |
| ------------------- | ---------------------------------------- | --------------------------------------------------- | ---------------------------------------------- |
| 后端单元测试        | API 控制器、鉴权、数据库操作             | CI `cargo test`                                     | 100% 关键路径通过，覆盖率 ≥70%                 |
| 前端单元 + 组件测试 | React hooks、状态管理、diff/log 组件     | CI `pnpm test`                                      | 所有断言通过；关键交互截图快照更新             |
| 端到端（E2E）       | 登录→任务创建→领取→执行→查看结果         | Playwright 脚本调用真实 API                         | 流程稳定通过，执行时间 <5 分钟                 |
| CLI 契约测试        | `codex cloud exec` / `codex cloud apply` | GitHub Actions 触发                                 | CLI 与 API 返回值一致，无 5xx                  |
| 执行器集成测试      | Supervisor + Firecracker snapshot        | `pytest -m executor`（或 `cargo test -p executor`） | 任务从 queued 到 succeeded，日志 diff 正确上传 |

### 11.2 手动验收清单

- [ ] 在全新 Ubuntu 22.04 主机上执行 `docker compose up -d`，确认全部容器启动成功。
- [ ] 通过 Web 端完成一次端到端任务执行，验证 diff 展示与日志滚动体验。
- [ ] 使用 CLI 指向私有 API，确认 `codex cloud exec` 能读取任务并提交结果。
- [ ] 强制重启主机，验证 systemd unit 将 Docker Compose 与 Ignite Supervisor 自动拉起。
- [ ] 断网场景：关闭外网，确认执行器仍能完成任务（依赖缓存生效）。
- [ ] 故障演练：手动删除 Firecracker snapshot，按照运维手册恢复并重新跑通任务。

### 11.3 性能与可靠性验证

- **启动性能**：
  - 使用 `hyperfine` 对比冷启动与 snapshot 启动时间，目标均值 <300 ms。
  - 记录测试数据并纳入文档，持续观察偏差。
- **并发压测**：
  - 通过 `k6` 或 Locust 对 API 施加 50 RPS，确保 p95 < 200 ms、无错误率飙升。
  - 使用自定义脚本批量投递 10 个并行任务，确认预热池策略不会饿死队列。
- **资源占用**：
  - 使用 `docker stats` 和 `ignite vm stats` 采样 CPU/Mem，确保单机资源在 32C/64G 内可控。
  - 建立报警阈值（CPU>85%、内存>80%、磁盘<20%）并测试告警触发。

### 11.4 安全校验

- 进行基础渗透测试（OWASP ZAP/ Burp Suite）验证登录、任务 API 无常见漏洞（SQL 注入、XSS、CSRF）。
- 执行器内检查：确认容器/VM 网络出站策略生效，仅允许白名单域名。
- 审核日志：验证关键操作（登录、任务状态变更、执行完成）均写入可检索日志。

## 13. 后续拓展方向

- 引入 RBAC、多租户与审计日志。
- 支持并行尝试、评审工作流、Webhook 通知。
- 完善可观测性：Prometheus 指标、集中日志、Trace。
- 优化执行器：缓存依赖、动态模型选择、成本控制。
- 支持水平扩展：将 Compose 拆分为多台主机，或迁移至 Kubernetes / Nomad。

---

该 MVP 设计确保在单台服务器上即可重现 Codex Cloud 的核心体验，前端、后端与执行器协同工作，并为后续企业级能力预留拓展空间。

## 13. 实施进展快照

- ✅ `cloud/backend` 提供 Rust (Axum) 单体服务，覆盖登录、仓库、任务、尝试、工件等 MVP API。
- ✅ `cloud/docker-compose.yml` + `Makefile dev` 单命令拉起 Axum API 容器，SQLite 与工件目录通过卷持久化。
- ✅ `cloud/backend/tests/task_flow.rs` 记录端到端 Happy Path，用于 CI 冒烟。
- ✅ `cloud/frontend` 提供 Next.js + Ant Design SPA，覆盖登录、任务列表、详情、创建与尝试提交流程。
- ⏳ Firecracker 执行器与 MinIO 接入仍在排期中。
