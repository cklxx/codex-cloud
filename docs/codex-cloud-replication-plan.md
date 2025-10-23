# Codex Cloud 自建方案设计

> 本文档旨在为希望在私有环境中复刻 Codex Cloud 能力的团队提供总体蓝图。内容基于开源 CLI 对托管服务的集成行为，结合推断出的云端特性，提出一套可落地的分层架构与实施路径。
>
> **进度提示**：团队当前聚焦在《[Codex Cloud 最小可行版本设计](./codex-cloud-mvp.md#11-实施启动计划)》所定义的单机 MVP 实施冲刺。本蓝图中的企业级能力暂缓，待 MVP 全量验收后再排期。
> 最新里程碑：后端骨架已在 `cloud/backend` 完成，Rust (Axum) 单体提供登录、任务流程与本地工件存储，作为后续多租户与审计能力的实现基线。

如需首先交付一个最小可行产品（MVP）以验证端到端能力，请先阅读《[Codex Cloud 最小可行版本设计](./codex-cloud-mvp.md)》，该文档聚焦于前端、后端与执行器的最小组合，并给出阶段性里程碑；本文档则覆盖完整的企业级功能图谱。

## 1. 目标与范围

- **功能目标**：实现与官方 Codex Cloud 类似的任务浏览、领取、执行、评审、合并等端到端体验，包括 Web 前端、任务编排、沙箱执行、补丁管理与审计日志。
- **运行环境**：以“单机 Docker Compose + Firecracker”MVP 为起点，支持向私有云或自有数据中心的 Kubernetes / 容器化运行模式演进。
- **安全合规**：满足企业级访问控制、操作审计、网络隔离需求，允许与现有身份提供方（IdP）集成。
- **非目标**：不复刻 OpenAI 内部的模型服务，实现侧需接入自定义的语言模型或复用 OpenAI API。

## 2. 能力映射与模块拆分

根据 CLI 暴露的 API 行为与交互流程，可将 Codex Cloud 能力拆分为以下核心域：

<!-- prettier-ignore -->
| 域 | 关键功能 | 核心接口 / 参考 | 自建要点 |
| --- | --- | --- | --- |
| 身份与工作区 | ChatGPT 登录态、ChatGPT-Account-Id 头 | `codex_cloud_tasks::init_backend` 中对会话与账户标识的加载流程 | 用企业 IdP + OIDC/OAuth2 替换，发放短期访问令牌与工作区上下文 |
| 任务目录 | 列表、筛选、排序、详情 | `CloudBackend::list_tasks`、`get_task_text`、`get_task_diff` 等接口 | 提供分页 API、状态机维护、支持审查视图 |
| 任务执行 | 尝试、最佳结果、预检 | `apply_task_preflight`、`apply_task`、`list_sibling_attempts` 等接口 | 引入作业编排器与沙箱执行器，支持多次尝试与评分 |
| 任务创建 | `codex cloud exec` 触发 | `CloudBackend::create_task`、环境选择与 label 解析 | 提供 API / Web 表单创建任务、预置环境标签与权限控制 |
| 工件与补丁 | Diff、文本片段、附件 | `get_task_diff`、`get_task_text` 等调用 | 统一存储补丁、日志、附件，支持渲染与下载 |

## 3. 总体架构

```
┌──────────────────────────────┐
│          Web 前端            │ ← React / Vue / Next.js，重现任务浏览器
└──────────────┬───────────────┘
               │GraphQL/REST
┌──────────────▼───────────────┐
│        API Gateway 层         │ ← 认证、速率限制、租户隔离
└──────────────┬───────────────┘
               │gRPC/事件
┌──────────────▼───────────────┐
│    Task Service（任务域）     │ ← 列表、筛选、状态机
├──────────────┬───────────────┤
│Workspace Svc │ Execution Svc │
│(仓库管理)    │(执行编排)      │
├──────────────┼───────────────┤
│Artifact Svc  │Notification   │
└──────────────┴───────────────┘
               │
        ┌──────▼──────┐
        │ 数据与存储 │ ← PostgreSQL + Redis + 对象存储
        └────────────┘
```

- **Web 前端**：参考 CLI 中的 Ratatui UI 流程，构建浏览器端页面。需实现任务列表、详情、diff 预览、执行尝试、终端输出流等组件。单机场景可直接复用 MVP 的 Next.js + Node 容器，后续按需替换为集中式静态资源服务或 CDN。
- **API Gateway**：统一校验身份、注入工作区上下文、做速率限制与审计。可选用 Kong / Envoy / 自研网关。
- **Task Service**：维护任务生命周期（Created → Ready → In Progress → Completed → Applied），提供过滤、排序、审查标记；持久化使用 PostgreSQL。
- **Workspace Service**：负责仓库克隆、快照、环境元数据维护，与 Execution Service 协同确定使用的代码版本与依赖。
- **Execution Service**：管理沙箱容器（Firecracker / Kubernetes Jobs），下载模型、执行指令，回传结果与日志。单机阶段使用 Ignite 驱动的 Firecracker microVM；规模化后可引入 Kubernetes Job / Nomad 以实现多节点调度。需支持“最佳结果 N”并行尝试。
- **Artifact Service**：存储 diff、日志、附件，提供签名下载 URL 与内联渲染。
- **Notification Service**：向邮件 / Slack / Webhook 推送任务状态、审批请求。

## 4. 关键技术决策

1. **身份与多租户**
   - 采用 OIDC/OAuth2（Azure AD、Okta、内部 SSO）。API Gateway 校验 JWT，提取 `account_id` 映射到内部用户与工作区。
   - 支持组织级 RBAC：角色（管理员、审查者、执行者、访客）与资源权限。
2. **执行隔离**
   - 单机阶段首选 Firecracker microVM，结合 Ignite snapshot 将启动时延压到 120~250 ms，并在 Supervisor 中预热 microVM 池以支撑突发任务。[^firecracker-plan]
   - 扩容到多节点时，可继续使用 Firecracker（Kubernetes KubeVirt / Kata Containers）或切换到 gVisor/Kata 以兼顾隔离与调度。
   - 通过网络策略限制外部访问，仅允许必要的包管理镜像或私有仓库；预构建运行时镜像缓存语言栈、CLI 工具与常用依赖。
3. **模型接入**
   - 提供统一的 Model Proxy Service，对接 OpenAI API、私有大模型或开源推理服务，暴露 `generate`, `plan`, `tool_call` 等接口，供 Execution Service 调用。
   - 支持 API Key / Token 注入，遵循最小权限原则。
4. **审批与审计**
   - 任务状态机引入“审批”节点，支持双人审查。
   - 所有操作写入审计日志（PostgreSQL + Elasticsearch），提供查询界面。
5. **可观测性**
   - Prometheus + Grafana 监控；集中日志（ELK / OpenSearch）。
   - 任务执行链路使用 OpenTelemetry Trace，关联用户操作、模型调用、容器执行。

## 5. 数据模型示例

- `users`、`organizations`、`memberships`：管理多租户。
- `tasks`：记录标题、描述、仓库、状态、优先级、创建人、受理人。
- `task_attempts`：包含执行日志、评分、耗时、模型版本。
- `artifacts`：diff、补丁、压缩包，存储对象存储中的路径与元数据。
- `environments`：环境标签、基础镜像、资源配额、访问策略。

## 6. 接口设计（REST/GraphQL 草案）

<!-- prettier-ignore -->
| 功能 | 方法与路径 | 请求示例 | 响应要点 |
| --- | --- | --- | --- |
| 列表任务 | `GET /v1/tasks?status=ready&workspace=foo` | 支持分页、模糊搜索 | 返回 summaries，与 CLI 期待结构对齐 (`TaskSummary`) |
| 获取详情 | `GET /v1/tasks/{id}` | | 包含描述、diff、附件链接、最近尝试 |
| 创建任务 | `POST /v1/tasks` | prompt、workspace、best_of_n | 返回 `task_id` 与初始 attempt |
| 提交尝试 | `POST /v1/tasks/{id}/attempts` | 环境、模型参数 | 触发 Execution Service | 
| 领取任务 | `POST /v1/tasks/{id}:claim` | | 设置执行人与过期时间 |
| 审批结果 | `POST /v1/tasks/{id}:approve` | comment、decision | 推进状态机 |
| Webhook | `POST /v1/hooks` | 注册回调 URL | 供外部系统订阅任务事件 |

## 7. 实施阶段

1. **阶段 0：准备**
   - 明确业务需求、指标（执行成功率、平均处理时长）。
   - 选定基础设施（Kubernetes 集群、CI/CD 工具、对象存储）。
2. **阶段 1：最小可用版本 (MVP)**
   - 搭建 Task Service + API + PostgreSQL。
   - 通过 CLI 的 `CloudBackend` mock 流程验证 API 对齐，确保 CLI 可以指向私有服务（调整 `CODEX_CLOUD_TASKS_BASE_URL`）。
   - 提供基础 Web UI（任务列表、详情、创建）。
3. **阶段 2：执行闭环**
   - 构建 Execution Service，接入模型代理，完成任务执行、日志上传、diff 生成。
   - 引入 Artifact Service + 对象存储。
   - 实现“最佳结果 N”并行尝试与评分策略。
4. **阶段 3：企业能力**
   - 集成组织管理、RBAC、审计日志。
   - 增加审批流、Webhook、通知。
   - 优化监控、告警、弹性伸缩。
5. **阶段 4：优化与扩展**
   - 支持多模型路由、成本分析、执行历史回放。
   - 引入自动回归测试、质量门禁，结合 CI/CD。

## 8. 部署与运维建议

- **基础设施即代码**：Terraform 管理云资源，Helm/Kustomize 部署微服务。
- **CI/CD**：GitOps（Argo CD / Flux）或流水线（GitHub Actions、GitLab CI）实现自动化部署。
- **备份策略**：数据库每日快照、对象存储版本化、日志归档。
- **灾备**：多区域部署或冷备，配置跨区域复制。
- **安全**：WAF 防护、API Token 轮换、密钥管理（HashiCorp Vault / KMS）。

## 9. 风险与缓解

<!-- prettier-ignore -->
| 风险 | 描述 | 缓解措施 |
| --- | --- | --- |
| 模型成本不可控 | 多次尝试会增加调用量 | 引入配额、成本仪表板、自动降级模型 |
| 沙箱逃逸 | 执行用户代码存在安全隐患 | 使用多重隔离 (VM + 网络策略)、持续安全审计 |
| 数据一致性 | 异步任务状态不同步 | 使用事件溯源 / outbox 模式，定期对账 |
| 用户体验差 | Web 端未复刻 CLI 的交互体验 | 复用 CLI 的交互流程、用户测试迭代 |

## 10. 交付物清单

- 架构设计文档（本文）
- 详细 API 规范与 OpenAPI 文档
- 数据库 ER 图与迁移脚本
- 执行环境镜像定义（Dockerfile + 镜像仓库）
- 运行手册（部署、扩缩容、应急）

## 11. 后续工作

- 依据本方案产出更详细的系统设计 (LLD) 与序列图。
- 选择并集成具体的模型服务，实现计划/执行模块。
- 规划安全评估与渗透测试，确保上线前通过合规审查。

[^firecracker-plan]: Firecracker "Performance" 文档指出通过 snapshot 恢复 microVM 的启动时延约 125 ms，详见 <https://github.com/firecracker-microvm/firecracker/blob/main/docs/performance.md>。
