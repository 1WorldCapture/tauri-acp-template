# ACP 客户端集成技术需求（模板级）

本文档用于定义本仓库模板在集成 Agent Client Protocol（ACP）时的**技术需求边界**与**最小能力集**。模板只提供可组合的原子能力与 UI 基础交互，不内置任何策略逻辑；策略由应用层实现。

## 0. 目标与原则

### 目标

- 在本 Tauri 模板中实现一个 **ACP Client**，通过 **STDIO 子进程**连接外部 ACP Agent Adapter（优先：Claude Code → Codex → Gemini CLI）。
- 提供可运行的最小闭环（E2E）：前端输入 prompt → Rust ACP → adapter → 返回流式/最终消息展示。
- 从架构上支持 **多个 Workspace 并行运行**，且每个 Workspace 内可并行运行多个 Agent 连接与操作。

### 模板原则（必须遵守）

- **原子化能力**：任何需要用户批准/可能产生副作用的操作，必须拆成原子操作（一次调用 = 一次用户可见动作 = 一个可追踪 ID）。
- **不做策略**：模板不内置“允许/拒绝/记忆/白名单/危险命令拦截/超时策略”等决策逻辑；策略由应用层实现。
- **每次都询问**：所有等同 MCP/工具权限的能力调用，模板默认 **每次询问用户**（仅提供 allow once / deny）。
- **并行优先**：架构必须支持多个 Workspace 并行，消息/状态不能串台。

## 1. 术语与角色

- **ACP Client**：本应用（Tauri）一侧，负责启动 adapter 子进程、发送 `initialize / session/new / session/prompt`，并处理 `session/update` 流式通知；同时实现 agent 反向调用的 client 能力（terminal、fs、权限等）。
- **ACP Agent（Adapter）**：外部 Node.js package（Zed 生态 adapter），通过 STDIO 与我们双向 JSON-RPC 通信。
- **Workspace**：用户选择的工作目录（根路径），作为 `cwd`、文件系统与终端执行的隔离边界。
- **Operation**：需要用户批准的原子动作实例（一次 terminal run、一次文件写入、一次安装 adapter 等），拥有唯一 `operationId`，贯穿 UI、权限、执行与取消。

## 2. 总体架构

### 分层

- Rust 后端（核心）
  - `WorkspaceRuntimeManager`：管理多个 Workspace runtime（并行隔离）。
  - `AcpProcessManager`：按（`workspaceId`, `agentId`）维度启动/重启/关闭 adapter 子进程（STDIO）。
  - `JsonRpcRouter`：双向 JSON-RPC（既能发请求，也能处理对方发来的请求）。
  - Capabilities：实现 client-side 的 `terminal/*`、`fs/*` 与权限请求处理与转发。
  - `EventEmitter`：把 `session/update`、tool call、terminal 输出、权限请求等事件推送给前端。
- React 前端（展示 + 用户交互）
  - Chat（文本输入/输出）
  - Tool Calls（列表 + 详情 + 状态）
  - Permissions（请求队列/弹窗）
  - Terminal（命令 + 输出 + stop/copy）

### 多 Workspace 并行模型

- 每个 Workspace 拥有独立的 runtime 状态（agent 连接、terminal 管理、权限队列、操作列表）。
- 所有事件/回调必须携带 `workspaceId`（并尽量携带 `sessionId / operationId / terminalId`），前端按 workspace 分桶渲染。

## 3. Workspace 需求

### 功能

- 支持创建/打开 Workspace（选择一个目录作为根）。
- 支持同时存在多个 Workspace（列表 + 切换）。
- 每个 Workspace 维护独立的 agent 连接集合（至少一个 active agent）。

### 安全边界（强制）

- 所有 `fs/*` 与 `terminal/*` 必须校验路径不越过 workspace root（包含符号链接与 `..` 解析后的真实路径）。
- `terminal.run` 默认 `cwd = workspace root`；如允许传子目录，必须仍在 workspace 内。

## 4. Adapter 支持与顺序

### 优先级（必须按此顺序落地）

1. Claude Code ACP adapter
2. Codex ACP adapter
3. Gemini CLI（ACP 模式/adapter）

### 运行方式

- 通过 **Node 子进程 + STDIO**运行 adapter 的可执行入口（bin）。

### 本地缓存安装（必须）

- 模板提供一个“adapter 本地缓存目录”（位于 app data/cache 下），用于安装 Node 包，确保可重复定位、离线复用。
- 模板提供原子操作：
  - `adapter.install(agentId, version?)`
  - `adapter.uninstall(agentId)`
  - `adapter.getStatus(agentId)`（已安装版本、路径、node 是否可用）
- install/uninstall 属于需要权限确认的原子操作（每次询问）。

### Node 运行时

- 需要探测 `node`/`npm` 可用性（路径、版本、错误提示）。
- 模板不负责登录/鉴权（由外部 CLI/adapter 自己处理）。

## 5. ACP 会话与消息流（STDIO）

### 基础流程（每个 workspace × agent 连接）

- spawn adapter 子进程（stdio）
- `initialize`（附 client capabilities）
- `session/new`（至少包含 `cwd = workspace root` 等）
- 每次用户输入：`session/prompt`，并处理 `session/update` 流式通知
- 结束：支持用户主动 stop（区分 stop turn / kill agent process / kill terminal process 等原子动作）

### 传输约束

- STDIO JSON-RPC 一条消息一行（`\n` 分隔）；stdout 必须保持干净（只输出协议消息）。

## 6. Terminal 能力（关键需求）

### 目标

- 用户不手动输入命令
- Agent 自动发起命令执行
- Agent 读取命令输出与退出信息
- UI 可见、可追踪、可停止、可复制

### 接口形态（已确认）

- `terminal.run` 接收**整段 shell 命令字符串**（降低 agent 生成门槛）。

### 执行模型

- 每次执行产生 `operationId` 与 `terminalId`（或 runId）。
- Rust spawn 子进程执行命令，流式采集 stdout/stderr：
  - 输出以 chunk 事件推送前端（Terminal 面板）
  - 同时通过 ACP 的 terminal 输出机制回传给 agent（让 agent“读结果”）
- 必须支持用户操作（模板级通用能力）：
  - `Stop`：kill/terminate 当前 terminal 子进程（原子操作）
  - `Copy`：复制命令、复制输出、复制 exit code、复制错误摘要

> 模板不内置策略（超时、并发上限、危险命令拦截、自动重试等）；应用层可在模板暴露的钩子/包装层实现。

## 7. 文件系统能力（fs/\*）

### 目标

- agent 能读写 workspace 内文件（用于生成/修改代码）
- 写操作必须显式权限确认（每次）

### 最小能力集（模板级）

- `fs.read_text_file`
- `fs.write_text_file`

### 约束

- 严格 workspace 边界校验。
- 写操作必须走 Operation 流：至少展示目标路径与写入摘要，并支持 Copy。

## 8. 权限系统（每次询问）

### 统一权限入口

- 任何涉及副作用的能力调用（terminal run/kill、fs write/delete、adapter install 等）都必须先走：
  - `requestPermission(operation)` → 前端 UI → 用户 allow once / deny → 后端执行或拒绝

### 模板不做策略

- 不提供 allow always / remember。
- 应用层如需记忆/分组/批量允许，在模板之上实现。

## 9. Tool Call 展示

- UI 必须展示 agent 的 tool call 流（pending/in_progress/completed/failed）。
- tool call 必须能关联到对应 `operationId`（terminal/fs/adapter/permission）。
- 每个 tool call/operation 必须支持 Copy（名称、参数摘要、结果摘要、错误）。

## 10. UI 最小需求（4 块面板）

1. Chat：文本输入框 + 消息列表；支持流式更新；支持 stop 当前 turn。
2. Tool Calls：列表 + 详情；显示状态与关联 operation；支持 Copy。
3. Permissions：请求队列/弹窗；展示“将要做什么、在哪个 workspace、由哪个 agent 发起”；allow once / deny。
4. Terminal：按 workspace/会话分组展示命令执行；实时 stdout/stderr；Stop/Copy。

## 11. 后端对前端的 API 形式（Tauri commands + events）

### Commands（示例方向，最终以实现为准）

- workspace：`createWorkspace/openWorkspace/closeWorkspace/listWorkspaces/setActiveWorkspace`
- agent：`listAgents/installAgent/startAgent/stopAgent/getAgentStatus`
- chat：`sendPrompt(workspaceId, agentId, prompt)`
- control：`stopTurn(workspaceId, sessionId)`、`killTerminal(workspaceId, terminalId)`

### Events（前端订阅）

- `acp/session_update`
- `acp/permission_requested`
- `terminal/output`
- `terminal/exited`
- `agent/status_changed`

> 所有事件必须携带 `workspaceId`，并尽量携带 `sessionId/operationId/terminalId` 以便 UI 精确路由。

## 12. 可观测性与调试

- 必须有统一日志（可开关）：
  - ACP 收发摘要（避免泄漏敏感信息）
  - 子进程生命周期（spawn/exit/code）
  - terminal 执行记录（命令、cwd、exit）
  - 权限决定（allow/deny、operationId）

## 13. 测试与质量门槛

- Rust：核心逻辑（workspace 路径校验、operation 流、jsonrpc framing）要有单元测试。
- TS：前端状态归并（tool calls / terminal output / permission queue）要有测试。
- 重大变更后运行：`npm run check:all`。

## 14. 验收标准（第一阶段）

- 支持创建多个 workspace，并能并行发送 prompt（至少两个 workspace 同时跑且互不串消息）。
- Claude Code adapter 可通过缓存安装并启动，完成一次对话闭环。
- agent 触发 terminal 执行：用户每次确认后执行；terminal 输出在 UI 可见；agent 能收到输出；用户可 stop。
- agent 触发 fs 写入：用户每次确认后写入 workspace 内文件。
- tool calls、permissions、terminal 输出均可追踪到 `workspaceId + operationId`。
