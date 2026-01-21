# ACP Phase 1（模板）集成详细设计方案（Tauri + Rust + React）

> 目标：基于 `docs/acp/technical-requirements.md` 在本模板中落地 **最小可运行闭环（E2E）**：前端输入 prompt → Rust ACP Client（STDIO）→ 外部 ACP Adapter（优先 Claude Code）→ 流式 `session/update` 回到前端展示；并具备 **多 Workspace 并行隔离**、**每次权限确认**、**terminal/fs 最小能力**、**可停止/可追踪的操作**。
> 参考实现来源：Zed 的 `agent_servers/src/acp.rs`（STDIO/initialize/session/回调）与 `acp_thread`（toolcall/permission/terminal/fs 状态模型与处理方式）。

---

## 1. 依赖分析（Dependency Analysis）

### 1.1 Rust 侧新增/确认依赖（Cargo）

**核心协议与序列化**

* `agent_client_protocol`（与 Zed 同源，用于 ACP 数据结构 + `ClientSideConnection` 连接器）
* `serde`, `serde_json`（事件 payload、协议对象序列化/反序列化）
* `specta`（模板已有，用于 TS bindings；新增 ACP bridge 类型需要 `Type` 派生）

**并发与异步**

* `tokio`（建议显式依赖：`process`, `io-util`, `sync`, `rt`；Tauri 默认 runtime 可用但建议明确）
* `futures` / `async-trait`（如果 `agent_client_protocol` 的 Client trait 需要 async trait；Zed 用了 `async_trait`）

**错误与日志**

* `anyhow`, `thiserror`（统一错误边界；便于给前端提供可读错误）
* `tracing` 或沿用 `log`（模板已有 `tauri_plugin_log`；保持一致即可）

**ID 与数据结构**

* `uuid`（生成 `workspaceId/operationId/terminalId` 等）
* `dashmap` 或 `parking_lot`（多 workspace 并发 map；Phase 1 可用 `tokio::sync::Mutex<HashMap<...>>` 但要关注锁粒度）

> 关键点：**不要直接把 `agent_client_protocol` 的类型暴露给 specta**（通常它们不实现 `specta::Type`），需要 **Bridge Types**（见 2.4）。

---

### 1.2 Node 运行时与 Adapter 安装依赖

Phase 1 要求 “缓存安装”：

* 依赖外部环境：`node`, `npm`（或 `pnpm`，但需求写 npm）
* 需要实现：

  * `adapter.getStatus(agentId)`：node/npm 可用性、已安装版本、bin 路径
  * `adapter.install(agentId, version?)`：在 app cache 下安装
  * `adapter.uninstall(agentId)`

> 不做鉴权：Claude/Codex/Gemini 的登录由外部 CLI 自己处理（需求已明确）。

---

### 1.3 Tauri 能力（Capabilities）与安全清单

模板是 Tauri（v2 风格）结构，新增 commands/events 后通常需要更新：

* `src-tauri/capabilities/*` 中的 allowlist（具体文件名视模板当前配置）
* 否则前端可能无法 invoke 新命令或监听新事件

---

### 1.4 前端依赖与状态管理

模板已有：

* Zustand（`src/store/ui-store.ts`）
* Vitest（`commands.test.ts`）
* i18next（命令搜索中用到）

Phase 1 新增建议：

* 新 store：`acp-store.ts`（workspace/chat/toolcalls/permissions/terminal 统一归并）
* 使用 `@tauri-apps/api/event` 订阅后端 events

---

## 2. 最小架构设计（Minimal Architecture Design）

### 2.1 与 Zed 架构映射（Adaptation Map）

| Zed 组件                                               | 作用                                                                                                    | 模板对应建议                                                           |
| ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `agent_servers/src/acp.rs`                           | spawn 子进程、STDIO JSON-RPC、initialize/session、实现 client 回调（permission/fs/terminal/session_notification） | `src-tauri/src/acp/process.rs` + `src-tauri/src/acp/delegate.rs` |
| `acp_thread::AcpThread`                              | 聚合 session updates → entries；toolcall 状态机；权限等待；terminal/fs 逻辑                                         | **前端 store 负责 UI 聚合** + 后端只保留“运行态/权限等待/terminal 进程”最小状态          |
| `ToolCallStatus/permission`                          | oneshot 等待用户选择                                                                                        | 后端 `PermissionManager` + 前端 Permissions 面板                       |
| `terminal.rs`                                        | terminal output 汇聚、exit status、user_stopped 标志                                                        | 后端 `TerminalManager`（收集输出 + kill + wait）                         |
| session_notification meta（terminal_info/output/exit） | agent 通过 meta 推送 terminal 展示                                                                          | 后端照搬解析，把 meta 转成事件（作为补充通道）                                       |

> **关键取舍（Phase 1）**：
>
> * Zed 把「消息/ToolCall/Plan 聚合」做在 Rust（AcpThread）。
> * 该模板更适合：Rust 负责 “连接/进程/权限等待/terminal/fs” ，React+Zustand 负责 “展示聚合”。
>   这样能减少后端 UI 状态机复杂度，同时满足 “模板不做策略” 的原则。

---

### 2.2 后端核心分层（建议模块）

在 `src-tauri/src/` 新增 `acp/` 模块（Phase 1 结构）：

```
src-tauri/src/acp/
  mod.rs
  types.rs          // Bridge types: ids, events, command payloads
  manager.rs        // WorkspaceRuntimeManager（多 workspace 并发）
  workspace.rs      // WorkspaceRuntime（root、agents、permissions、terminals）
  process.rs        // AcpProcessManager（spawn/kill/restart + stdio）
  connection.rs     // AcpConnection（wrap agent_client_protocol::ClientSideConnection）
  delegate.rs       // ClientDelegate：request_permission/fs/terminal/session_notification
  permissions.rs    // PermissionManager：队列 + oneshot 等待
  terminal.rs       // TerminalManager：run/kill/output/wait
  fs.rs             // FsManager：read/write + workspace 边界校验
  adapter.rs        // AdapterManager：install/uninstall/status（cache dir）
  path.rs           // workspace path 安全校验工具（canonicalize、symlink、..）
```

同时新增 Tauri commands：

* `src-tauri/src/commands/acp.rs`（或拆分 workspace/agent/chat/control）

并更新：

* `src-tauri/src/commands/mod.rs`：`pub mod acp;`
* `src-tauri/src/bindings.rs`：collect 新 commands
* `src-tauri/src/lib.rs`：`.setup()` 中 `app.manage(WorkspaceRuntimeManager::new(...))`

---

### 2.3 多 Workspace 并行模型（Phase 1 必须满足）

**数据模型（Rust 侧）**

* `WorkspaceRuntimeManager`

  * `workspaces: HashMap<WorkspaceId, Arc<WorkspaceRuntime>>`
* `WorkspaceRuntime`

  * `root_dir: PathBuf`（canonicalized）
  * `agents: HashMap<AgentId, AgentRuntime>`（同一 workspace 内可多个 agent）
  * `permission_manager: PermissionManager`
  * `terminal_manager: TerminalManager`
  * `fs_manager: FsManager`
* `AgentRuntime`

  * `process: AcpProcessHandle`（child + io tasks）
  * `connection: AcpConnectionHandle`（initialize + sessions）
  * `sessions: HashMap<SessionId, SessionRuntime>`（Phase 1 可只维护 active session）
* `SessionRuntime`

  * `session_id: String`
  * `agent_id: AgentId`
  * `workspace_id: WorkspaceId`

**并行隔离约束（需求强制）**

* 所有事件 payload 必带 `workspaceId`，尽量带 `sessionId / operationId / terminalId`
* 所有 fs/terminal 操作必须做 workspace root 边界校验（含 symlink 与 `..`）

---

### 2.4 Bridge Types（强烈建议：协议类型不要直接暴露给前端）

因为 `agent_client_protocol` 的类型通常不实现 `specta::Type`，建议在 `src-tauri/src/acp/types.rs` 定义**前端友好的桥接类型**，只覆盖 Phase 1 用到的字段。

#### 2.4.1 ID 类型

```rs
pub type WorkspaceId = String; // uuid string
pub type AgentId = String;     // "claude" | "codex" | "gemini" (Phase 1)
pub type SessionId = String;   // acp::SessionId.0
pub type ToolCallId = String;  // acp::ToolCallId.0
pub type OperationId = String; // tool_call_id or uuid
pub type TerminalId = String;  // acp terminal id string / uuid
```

#### 2.4.2 事件类型（后端 → 前端）

建议事件统一走一个频道，或按需求拆分频道（需求示例给了多个事件名）。Phase 1 推荐按需求拆分，便于前端订阅与路由：

* `acp/session_update`
* `acp/permission_requested`
* `terminal/output`
* `terminal/exited`
* `agent/status_changed`

对应 payload（示意）：

```rs
#[derive(Serialize, Type)]
pub struct AcpSessionUpdateEvent {
  pub workspace_id: WorkspaceId,
  pub agent_id: AgentId,
  pub session_id: SessionId,
  pub update: AcpSessionUpdate, // Bridge enum
}

#[derive(Serialize, Type)]
#[serde(tag="type")]
pub enum AcpSessionUpdate {
  UserMessageChunk { text: String },
  AgentMessageChunk { text: String },
  AgentThoughtChunk { text: String },
  ToolCall { tool_call: AcpToolCall },
  ToolCallUpdate { update: AcpToolCallUpdate },
  Plan { entries: Vec<AcpPlanEntry> },
  // 兜底
  Unknown { raw: serde_json::Value },
}
```

权限请求事件：

```rs
#[derive(Serialize, Type)]
pub struct AcpPermissionRequestedEvent {
  pub workspace_id: WorkspaceId,
  pub agent_id: AgentId,
  pub session_id: SessionId,
  pub operation_id: OperationId,     // 建议 = tool_call_id
  pub tool_call_id: ToolCallId,
  pub title: Option<String>,
  pub summary: String,               // 前端展示用摘要（不做策略，只做渲染）
  pub options: Vec<AcpPermissionOption>,
}
```

terminal 事件：

```rs
#[derive(Serialize, Type)]
pub struct TerminalOutputEvent {
  pub workspace_id: WorkspaceId,
  pub terminal_id: TerminalId,
  pub operation_id: Option<OperationId>,
  pub stream: TerminalStream, // stdout/stderr
  pub chunk: String,          // utf-8 尝试解码；不可解码则 base64
}

#[derive(Serialize, Type)]
pub struct TerminalExitedEvent {
  pub workspace_id: WorkspaceId,
  pub terminal_id: TerminalId,
  pub operation_id: Option<OperationId>,
  pub exit_code: Option<i32>,
  pub signal: Option<String>,
  pub user_stopped: bool,
}
```

> 这套 bridge 的关键目的：
>
> * 前端 store 不需要理解完整 ACP 协议，只要理解 Phase 1 子集
> * 后续 Phase 2/3 可以扩展 enum，不破坏已有 UI

---

### 2.5 JSON-RPC / STDIO 连接设计（复用 Zed 模式）

#### 2.5.1 连接层（推荐直接复用 `ClientSideConnection`）

参照 `zed/crates/agent_servers/src/acp.rs`：

* `spawn adapter` 子进程：stdin/stdout/stderr piped

* 用 `agent_client_protocol::ClientSideConnection::new(delegate, stdin, stdout, spawn_fn)`

  * `delegate`：实现 client 回调（permission/fs/terminal/session_notification）
  * `spawn_fn`：把 future 派发到 tauri async runtime（类似 Zed 用 foreground executor）

* `initialize`：

  * `ProtocolVersion::V1`
  * `client_capabilities` 至少：

    * fs：read_text_file/write_text_file = true
    * terminal = true
  * meta（可选，但 Zed 用于 terminal 渲染实验特性）：

    * `terminal_output`, `terminal-auth`（是否需要取决于 adapter；Phase 1 可保留以兼容）

* `new_session(cwd=workspace_root)`

* `prompt(session_id, content_blocks)`；处理 stream updates（通过 `session_notification` 回调来接收）

#### 2.5.2 STDIO framing 要点（需求强制）

* 一条 JSON-RPC 一行，`\n` 分隔
* stdout 必须保持干净：不要混入日志；日志走 stderr 或 tauri log target
* stderr 单独读，写到 log（Zed `stderr_task` 模式）

---

### 2.6 权限系统（每次询问，不做策略）

**后端：PermissionManager**

* 每个 workspace 一个 permission manager（避免串台）
* 核心职责：

  1. 收到 ACP `request_permission` → 生成 `operationId`（建议 = `tool_call_id`）
  2. `emit acp/permission_requested` 给前端（带 options）
  3. 用 `oneshot` 等待前端 `respondPermission(...)`
  4. 返回 ACP `RequestPermissionResponse` 给 adapter（AllowOnce / Deny）

**前端：Permissions 面板**

* 展示：将要做什么、在哪个 workspace、哪个 agent/session
* 按需求只提供：`allow once / deny`
* 用户选择后 invoke：`acp.respond_permission(workspaceId, operationId, optionId)`

> Zed 对应点：`request_tool_call_authorization`（oneshot）+ `authorize_tool_call`（选项回传）

---

### 2.7 Terminal 子系统（run/stop/stream/copy + agent 读结果）

**后端：TerminalManager（每 workspace）**

* `create_terminal`：生成 `terminalId`，spawn 子进程（默认 cwd = workspace root）
* 采集 stdout/stderr chunk：

  * `emit terminal/output` 给前端（实时）
  * 内部保留 output buffer（可做 byte limit，参考 Zed `output_byte_limit`）
* `kill_terminal(terminalId)`：终止子进程
* `terminal_output(terminalId)`：返回当前输出（供 agent 调用）
* `wait_for_terminal_exit(terminalId)`：await exit status
* `user_stopped` 标志：用户点 Stop 触发（参考 Zed `stop_by_user/was_stopped_by_user`）

**与 ACP 方法的对应（以 Zed 能力为基线）**

* 实现（至少）：

  * `create_terminal`
  * `kill_terminal_command`
  * `release_terminal`
  * `terminal_output`
  * `wait_for_terminal_exit`

**关于“terminal.run 接收整段命令字符串”的落地方式**

* UI/permission 摘要与 Terminal 面板都以 **单字符串命令**展示
* 执行层可兼容两种输入：

  1. adapter 调 `create_terminal(command, args)`：展示/执行时拼接成一条
  2. 若 adapter 使用 “单字符串 command”（args 为空）：直接用 shell 执行（如 `sh -lc <command>` / `cmd /C <command>`）

---

### 2.8 文件系统子系统（fs.read/write + workspace 边界）

**后端：FsManager（每 workspace）**

* `read_text_file(path, line?, limit?)`：读取 workspace 内文件文本（行数截断）
* `write_text_file(path, content)`：写入文本（每次权限确认由 request_permission 驱动）
* **强制**：路径不能越过 workspace root（包括 symlink 与 `..`）

建议提供统一校验函数：

```rs
fn resolve_path_in_workspace(root: &Path, input: &Path) -> Result<PathBuf, PathError>
```

**策略边界**

* 不做危险命令/路径黑名单等策略（需求明确）
* 只做“越界校验 + 错误透明化”

---

### 2.9 Adapter 缓存安装（Phase 1 必须）

**后端：AdapterManager**

* cache dir：`app_cache_dir()/acp_adapters/<agentId>/`
* 安装方式：

  * 生成/维护 `package.json`（最小依赖）
  * `npm install <pkg>@<version>`（可选 version）
  * 解析可执行 bin 路径（node_modules/.bin/xxx 或 package bin 字段）

**原子操作（Operation）**

* install/uninstall/status 都作为 Operation：

  * install/uninstall 需要权限确认（每次询问）
  * getStatus 不需要

---

## 3. 实施路径（Implementation Path，逐步落地）

> 本节按“最小闭环优先 + 风险从底层到上层”排序。每一步列出需要改动的文件、关键新增接口与影响范围。

---

### Step 0：建立 ACP 模块骨架 + AppState 注入

**改动文件**

* `src-tauri/src/lib.rs`

  * `.setup(|app| { ... })` 内 `app.manage(WorkspaceRuntimeManager::new(app.handle().clone()))`
* `src-tauri/src/acp/mod.rs`（新增）
* `src-tauri/src/acp/manager.rs`（新增）
* `src-tauri/src/acp/types.rs`（新增）

**关键接口**

* `WorkspaceRuntimeManager::new(app: AppHandle) -> Self`
* `WorkspaceRuntimeManager` 提供对外 API（被 commands 调用）：

  * `create_workspace(root: PathBuf) -> WorkspaceId`
  * `close_workspace(id: WorkspaceId)`
  * `list_workspaces() -> Vec<WorkspaceSummary>`
  * `set_active_workspace(id)`

**影响**

* 建立后端全局状态（Tauri State），为后续 process/terminal/fs 共享提供入口

---

### Step 1：Workspace 安全边界（路径解析工具）

**新增文件**

* `src-tauri/src/acp/path.rs`
* 可在 `src-tauri/src/types.rs` 或 `acp/types.rs` 定义错误类型 `WorkspacePathError`

**实现设计要点（不写实现代码，但定义行为）**

* `canonicalize_root(root)`：workspace root 必须存在、可 canonicalize
* `resolve_target(root, path)`：

  * 若 `path` 是相对路径：`root.join(path)` 再 clean
  * 若 `path` 是绝对路径：允许，但必须落在 root 内
  * 写文件时目标可能不存在：canonicalize parent + join filename 的方式校验
* 处理 symlink：校验使用真实路径（canonicalized）前缀匹配

**影响**

* 这是 terminal/fs 的共同基础；必须先落地

---

### Step 2：AdapterManager（install/uninstall/status）与缓存目录约定

**新增文件**

* `src-tauri/src/acp/adapter.rs`

**新增 Tauri commands（先做 status，后做 install）**

* `acp_get_agent_status(agentId) -> AgentStatus`
* `acp_install_agent(agentId, version?) -> OperationStarted`
* `acp_uninstall_agent(agentId) -> OperationStarted`

**Operation 与 Permission**

* install/uninstall 触发 `acp/permission_requested`（operationId = uuid）
* 前端通过 `acp_respond_permission(...)` 决定 allow/deny

**关键设计决策**

* “Adapter 定位方式”：

  * **推荐**：把 bin path 固化到 `AgentStatus`，`start_agent` 直接用该 bin 启动
* “npm 执行方式”：

  * 使用 `tokio::process::Command` 直接 spawn `npm`（跨平台要处理 PATH）

---

### Step 3：AcpProcessManager（spawn/kill/restart）与 stderr 清洁

**新增文件**

* `src-tauri/src/acp/process.rs`

**核心结构**

* `AcpProcessHandle`：

  * `child: tokio::process::Child`
  * `stdin: ChildStdin`
  * `stdout: ChildStdout`
  * `stderr_task: JoinHandle<()>`
  * `wait_task: JoinHandle<()>`

**行为约束**

* stderr 全部写日志（warn 或 debug）
* stdout 只交给 JSON-RPC/ACP connection 读取

**影响**

* 为 `agent_client_protocol::ClientSideConnection` 提供 io handle

---

### Step 4：AcpConnection（initialize/new_session/prompt/cancel）

**新增文件**

* `src-tauri/src/acp/connection.rs`
* `src-tauri/src/acp/delegate.rs`

**核心职责**

* 包装 `agent_client_protocol::ClientSideConnection`
* 管理 session 生命周期（Phase 1：每 workspace 每 agent 至少一个 active session）
* 将 session updates 转换为 bridge event 并 emit

**关键接口（示例签名）**

```rs
impl AcpConnection {
  async fn start(
    workspace_id: WorkspaceId,
    agent_id: AgentId,
    root_dir: PathBuf,
    adapter_cmd: AdapterCommand, // path + args + env
    delegate: ClientDelegateHandle,
  ) -> Result<Self>;

  async fn new_session(&self) -> Result<SessionId>;

  async fn prompt(&self, session_id: &SessionId, text: String) -> Result<()>;

  fn cancel(&self, session_id: &SessionId) -> Result<()>;

  async fn shutdown(&self) -> Result<()>;
}
```

**initialize 的能力声明**

* fs read/write = true
* terminal = true
* meta：可保留 Zed 的 `terminal_output`、`terminal-auth`（兼容性优先）

---

### Step 5：ClientDelegate（permission/fs/terminal/session_notification）

**新增文件**

* `src-tauri/src/acp/delegate.rs`
* `src-tauri/src/acp/permissions.rs`
* `src-tauri/src/acp/fs.rs`
* `src-tauri/src/acp/terminal.rs`

**Delegate 需要实现的回调（对齐 Zed 版本）**

* `request_permission(...) -> RequestPermissionResponse`

  * 生成 permission event → await 前端响应 → 返回 outcome
* `read_text_file(...)`

  * FsManager + 路径校验
* `write_text_file(...)`

  * FsManager + 路径校验
* `create_terminal(...)`

  * TerminalManager.spawn → 返回 terminalId
* `kill_terminal_command(...)`

  * TerminalManager.kill
* `release_terminal(...)`

  * TerminalManager.release
* `terminal_output(...)`

  * TerminalManager.current_output
* `wait_for_terminal_exit(...)`

  * TerminalManager.wait_exit
* `session_notification(...)`

  * 将 `SessionUpdate` 转为 `AcpSessionUpdateEvent` emit 给前端
  * **额外兼容**：解析 toolcall meta（参考 Zed）

    * `terminal_info`：可提前注册 terminal（display-only 或 placeholder）
    * `terminal_output`：emit `terminal/output`（如果 agent 通过 meta 推送）
    * `terminal_exit`：emit `terminal/exited`

> 这一步是 Phase 1 的“协议闭环核心”，也是最接近 Zed 的部分。

---

### Step 6：WorkspaceRuntimeManager 串起 workspace × agent × session

**改动/新增**

* `src-tauri/src/acp/workspace.rs`
* `src-tauri/src/acp/manager.rs`

**对外能力**

* `open_workspace(root_dir) -> WorkspaceId`（创建 runtime）
* `start_agent(workspaceId, agentId) -> AgentStarted`（spawn adapter + initialize + new_session）
* `send_prompt(workspaceId, agentId, prompt) -> ()`
* `stop_turn(workspaceId, sessionId) -> ()`
* `stop_agent(workspaceId, agentId) -> ()`
* `kill_terminal(workspaceId, terminalId) -> ()`

**关键设计点**

* `start_agent` 内部确保：

  1. adapter 已安装（否则提示）
  2. spawn process
  3. initialize
  4. new_session（cwd = workspace root）
  5. emit `agent/status_changed`

---

### Step 7：Tauri Commands 与 Specta bindings 汇总

**改动文件**

* `src-tauri/src/commands/mod.rs`：增加 `pub mod acp;`
* `src-tauri/src/commands/acp.rs`（新增）
* `src-tauri/src/bindings.rs`：collect commands
* `src-tauri/src/types.rs`：如需复用错误类型，可拆到 `acp/types.rs`

**建议命令清单（Phase 1）**
Workspace：

* `acp_create_workspace(root: String) -> WorkspaceId`
* `acp_list_workspaces() -> Vec<WorkspaceSummary>`
* `acp_set_active_workspace(workspaceId) -> ()`
* `acp_close_workspace(workspaceId) -> ()`

Agent：

* `acp_list_agents() -> Vec<AgentDescriptor>`
* `acp_get_agent_status(agentId) -> AgentStatus`
* `acp_install_agent(agentId, version?) -> OperationId`
* `acp_uninstall_agent(agentId) -> OperationId`
* `acp_start_agent(workspaceId, agentId) -> SessionId`
* `acp_stop_agent(workspaceId, agentId) -> ()`

Chat/Control：

* `acp_send_prompt(workspaceId, agentId, sessionId, prompt) -> ()`
* `acp_stop_turn(workspaceId, sessionId) -> ()`
* `acp_kill_terminal(workspaceId, terminalId) -> ()`

Permissions：

* `acp_respond_permission(workspaceId, operationId, optionId) -> ()`

---

### Step 8：前端最小 UI 与状态归并（4 面板）

**新增建议目录**

```
src/store/acp-store.ts
src/lib/acp/client.ts            // 封装 tauri bindings + 事件订阅
src/components/acp/ChatPanel.tsx
src/components/acp/ToolCallsPanel.tsx
src/components/acp/PermissionsPanel.tsx
src/components/acp/TerminalPanel.tsx
```

**store 结构（建议）**

* `workspaces: Record<workspaceId, WorkspaceState>`
* `activeWorkspaceId`
* `WorkspaceState`：

  * `sessions: Record<sessionId, SessionState>`
  * `permissionsQueue: PermissionRequest[]`
  * `terminals: Record<terminalId, TerminalState>`
  * `toolCalls: Record<toolCallId, ToolCallState>`
  * `messages: ChatMessage[]`（按 session 分桶）

**事件归并逻辑**

* `acp/session_update`：

  * chunk 累加到 message
  * toolcall upsert + status 更新
  * plan 更新（Phase 1 可先简单展示列表）
* `acp/permission_requested`：

  * push 到 `permissionsQueue`
* `terminal/output`：

  * append 到 terminal buffer
* `terminal/exited`：

  * 标记 terminal 完成，显示 exit code
* `agent/status_changed`：

  * 展示安装/运行态

**与模板命令系统结合（可选但有利于测试）**

* 在 `src/lib/commands` 新增 “ACP: Send Prompt / Stop Turn / Switch Workspace” 等 command
* 复用 `use-command-context.ts` 的 `showToast` 用于错误提示（例如 agent 未安装）

---

## 4. 测试策略（Testing Strategy）

> 需求明确要求包含测试策略；这里按“对架构有影响的测试点”来设计。不会写具体实现，但定义测试层级与关键夹具（fixtures）。

### 4.1 Rust 单元测试（核心逻辑）

**目标文件**

* `src-tauri/src/acp/path.rs`
* `src-tauri/src/acp/permissions.rs`
* `src-tauri/src/acp/jsonrpc`（如果你实现了自定义 framing；若完全依赖 ACP crate，可测 delegate 映射）
* `src-tauri/src/acp/terminal.rs`（尽量写可控的纯逻辑测试）

**测试重点**

1. **workspace 边界校验**（最关键）

   * `..` 越界
   * symlink 指向 workspace 外
   * 写入新文件（目标不存在）时的 parent 校验
2. **PermissionManager 正确性**

   * 多 workspace 并行：A workspace 的 respond 不影响 B
   * 超时/取消（如 stop_turn 时取消 pending permission）行为定义
3. **事件 payload 结构稳定**

   * `AcpSessionUpdate::Unknown` 兜底不 panic
   * toolcall/status 解析兼容

---

### 4.2 Rust 集成测试（协议闭环，无需真实 Claude/Codex）

**关键建议：提供 Stub ACP Adapter**

* 用一个最小 Node/Rust “假 adapter”：

  * 能响应 `initialize/new_session/prompt`
  * 能发送 `session_notification`（模拟流式 chunk + toolcall + toolcall_update）
  * 能触发 `request_permission` 并等待客户端响应
  * 能调用 `write_text_file/create_terminal/wait_for_terminal_exit`

**验证点（Phase 1 验收对齐）**

* 同时创建两个 workspace，各自启动 stub agent，**并行 prompt** 不串台
* permission 流：前端 respond 后，adapter 能继续执行
* terminal 输出：能产生 `terminal/output` 与 `terminal/exited`，并可 stop
* fs 写入：workspace 内可写；workspace 外必失败

> 这类测试决定你是否需要把 runtime manager 做成可注入（dependency injection），例如把 “emit event” 抽象成 trait，以便测试中捕获事件而不依赖真实 window。

---

### 4.3 前端（Vitest）状态归并测试

参考现有 `src/lib/commands/commands.test.ts` 写法，新增：

**目标文件**

* `src/store/acp-store.ts`
* `src/lib/acp/client.ts`（事件订阅层可 mock）

**测试重点**

1. `acp/session_update` chunk 累加逻辑（assistant/user）
2. toolcall upsert 与 status 更新逻辑（Pending/InProgress/Completed/Failed）
3. permission queue 入队/出队（allow/deny 后移除）
4. terminal output buffer append 与 exited 标记

**Mock 策略**

* Mock `@tauri-apps/api/event` 的 `listen`，直接注入事件 payload
* Mock `bindings.ts` commands 调用

---

### 4.4 最小 E2E（命令驱动）

利用模板已有 command system 的思路：

* 新增一个 “ACP: Send Prompt” command → 触发 `acp_send_prompt`
* 在测试里 mock 后端返回的 `acp/session_update` 事件流
* 验证：UI store 最终形成消息列表、toolcall 列表、terminal 输出

> 这能在不引入 Playwright 的情况下，验证 “命令 → store → 渲染数据结构” 的闭环正确性。

---

## 5. 关键架构决策清单（Phase 1 必须尽早确定）

1. **是否完全复用 `agent_client_protocol::ClientSideConnection`**

   * 推荐复用（对齐 Zed，减少协议坑）
   * 若自写 JSON-RPC router，需要额外测试成本与 bug 风险

2. **消息/ToolCall 聚合放后端还是前端**

   * 推荐 Phase 1 放前端（后端专注 runtime）
   * 后端仅提供：session updates（bridge）、权限等待、terminal/fs 实际执行

3. **operationId 与 tool_call_id 的关系**

   * 推荐：工具调用类 operationId = tool_call_id（天然可追踪/无需额外映射）
   * install/uninstall 用 uuid（与 toolcall 无关）

4. **terminal.run “单字符串命令” 的兼容**

   * UI 展示统一单字符串
   * 执行层兼容 create_terminal(command,args) 与 command-only 两种输入

5. **capabilities allowlist 更新策略**

   * 每新增一个 command/event，都要同步更新 Tauri capabilities（否则前端不可用）

---

## 6. Phase 1 产出物与验收映射（对齐技术需求 14）

* ✅ 多 workspace 并行：`WorkspaceRuntimeManager` + 事件包含 `workspaceId`
* ✅ Claude Code adapter 缓存安装：`AdapterManager` + install/getStatus
* ✅ 对话闭环：initialize → new_session → prompt → session_update 流式事件
* ✅ terminal：权限确认后执行；输出可见；可 stop；agent 可读 output/exit
* ✅ fs write：权限确认后写入；严格 workspace 边界
* ✅ toolcalls/permissions/terminal 关联：tool_call_id / operationId / terminalId 全链路可追踪

---
