# Tauri + ACP 模板框架 PRD（需求文档）

## 1. 背景与目标

### 1.1 背景

你希望搭建一个 **跨平台桌面应用模板**，以便未来快速启动项目。技术核心是：

* **Tauri（Rust 后端 + Web UI）**
* **前端工程化（React / Vite / Tailwind）**
* **本地 Agent 调用**：不直接对接各家 Provider API，而是通过 **Agent Client Protocol（ACP）** 调用本地 Agent CLI（例如 Codex、Gemini CLI、Claude Code），由 CLI 自己负责鉴权与模型调用。

ACP 是一个用于 Client（编辑器/IDE/客户端）与 Agent（编码/智能体程序）通信的协议，采用 JSON-RPC 2.0，并支持本地 stdio 子进程模式（以及远端模式草案）。([Agent Client Protocol][1])

### 1.2 模板的核心目标（只做“框架骨架”）

1. **提供一个可直接运行的 Tauri 工程骨架**（前端 + Rust 后端 + 基础路由/页面）
2. **前端集成 React/Vite/Tailwind + i18n + TypeCheck/Lint 全套**
3. **主题系统（可替换主题 + Day/Night）**
4. **ACP Client（在 Rust 侧实现）**：可配置地启动本地 ACP Agent（stdio），完成最小对话闭环
5. **端到端最小对话 Demo**：前端输入 prompt → Rust → ACP → Agent → 返回消息显示

---

## 2. 范围界定

### 2.1 本期范围（In Scope）

#### A. UI/前端工程化

* React + Vite + Tailwind CSS
* i18n（至少内置 `zh-CN` / `en-US` 两套资源）
* TypeCheck + Lint + 格式化 + Git hooks（可选）+ CI（可选但推荐）

#### B. 主题系统

* 基于“主题包/主题 token”的架构
* 支持 Day/Night（亮色/暗色）切换
* 支持后续新增/替换主题，不需要大改业务代码

#### C. ACP + Agent 集成

* Rust 后端实现 **ACP client（stdio）**，可启动本地 Agent 子进程
* 支持多 Agent 配置（至少预置：Gemini CLI、Claude Code、Codex 的示例配置项）
* 支持 ACP 最小会话流程：`initialize` → `session/new` → `session/prompt`，并处理 `session/update` 通知，完成消息回传 ([Agent Client Protocol][2])

#### D. 最小端到端对话（E2E）

* 前端：输入框 + 消息列表
* 后端：接收 prompt、调用 ACP、把回复推送回前端（支持流式 chunk 的话更贴合 ACP 的 `agent_message_chunk`）([Agent Client Protocol][3])

---

### 2.2 明确不做（Out of Scope / Non-goals）

为避免“模板不切实际的虚化需求”，以下明确不做：

* 不承诺或评估 **性能、可扩展性、稳定性** 之类指标
* 不做复杂的“多窗口/多页面应用框架”
* 不做完整编辑器能力（文件树、diff 审核、终端集成、MCP 工具调用 UI、权限弹窗体系等）
* 不直接集成 Provider API（OpenAI/Google/Anthropic），鉴权由本地 CLI/Adapter 负责
* 不做“插件市场/可插拔 Agent 商店”等产品化功能

---

## 3. 目标用户与使用场景

### 3.1 目标用户

* 主要用户：**未来使用该模板启动新桌面应用项目的开发者（你自己或团队）**
* 需求重点：**开箱即用的工程骨架 + 清晰的扩展点**

### 3.2 典型使用场景

1. 开发者 `git clone template` 后，安装依赖并启动 Tauri Dev
2. 在 UI 中选择一个本地 Agent（如 Gemini CLI / Claude Code / Codex）
3. 输入一段文本 prompt
4. 收到 agent 回复（显示为对话消息）

---

## 4. 总体架构概览（模板层面的“约定”）

### 4.1 模块分层

* **Frontend（React）**

  * Chat UI（输入/消息列表）
  * 主题切换 / 语言切换
  * 调用 Tauri command（invoke）发送 prompt
  * 监听 Tauri event 接收流式/最终回复
* **Backend（Rust / Tauri）**

  * ACP Client 模块：管理 agent 子进程、JSON-RPC 消息收发、会话生命周期
  * Tauri commands：`send_prompt`、`switch_agent`、`list_agents` 等
  * 事件推送：`agent_update`（chunk）、`agent_done`（结束）、`agent_error`

### 4.2 端到端数据流（最小闭环）

```
[React UI] --invoke(sendPrompt)--> [Rust/Tauri Command]
   [Rust] --spawn ACP agent subprocess--> [Agent CLI]
   [Rust] --JSON-RPC initialize/session/new/session/prompt--> [Agent]
   [Agent] --session/update (agent_message_chunk)--> [Rust]
   [Rust] --emit event--> [React UI 更新消息]
   [Agent] --session/prompt response stopReason--> [Rust]
   [Rust] --emit done--> [React UI 完成一轮消息]
```

ACP 的典型消息流包含初始化、创建会话、prompt 轮次与 `session/update` 通知（用于计划/文本 chunk/tool call 状态等）。([Agent Client Protocol][4])
stdio 传输要求消息为 UTF-8 JSON-RPC，**以换行分隔且消息不得包含内嵌换行**（因此必须发送单行 JSON）。([Agent Client Protocol][5])

---

## 5. 功能需求

### 5.1 工程骨架

#### 5.1.1 仓库结构（建议约定，不强制）

* `/src-tauri/`：Tauri Rust 后端
* `/src/`：React 前端源码
* `/src/features/chat/`：对话相关 feature
* `/src/features/settings/`：主题/语言/agent 选择
* `/src/shared/`：通用组件、hooks、工具函数
* `/acp/agents.json`（或 `.toml`）：Agent 配置文件（命令、参数、环境变量等）

#### 5.1.2 一键启动脚本

* `dev`：启动前端 + Tauri
* `build`：构建前端 + Tauri
* `lint` / `typecheck`：工程质量检查

---

### 5.2 UI 技术栈集成（React + Vite + Tailwind）

#### 5.2.1 技术要求

* React（函数组件 + hooks）
* Vite（构建/Dev Server）
* Tailwind CSS（原子化样式）
* TypeScript（默认启用严格模式）

#### 5.2.2 UI 页面/组件最小集

* **主界面（单页即可）**

  * 顶部/侧边：Agent 选择、主题切换、语言切换入口
  * 主区：消息列表（User/Agent）
  * 底部：输入框 + 发送按钮 +（可选）停止按钮

---

### 5.3 国际化（i18n）

#### 5.3.1 能力要求

* 内置 `zh-CN`、`en-US` 两种语言资源
* 运行时切换语言
* 语言设置持久化（localStorage 或 Tauri store）

#### 5.3.2 文案覆盖范围（最小）

* App 标题
* 发送按钮、输入 placeholder
* Agent 选择相关提示
* 错误提示（Agent 未安装/启动失败/协议握手失败等）
* 主题/语言切换文案

---

### 5.4 TypeCheck 与 Lint 工具链

#### 5.4.1 前端（必选）

* TypeScript typecheck：`tsc --noEmit`
* ESLint：包含 TS、React hooks、基础最佳实践
* Prettier：格式化统一
  -（可选）Stylelint：如你希望对 CSS/Tailwind 层也做 lint

#### 5.4.2 Rust（推荐）

* `cargo fmt`（格式化）
* `cargo clippy`（lint）

#### 5.4.3 CI（推荐但不强制）

* GitHub Actions：PR 时跑

  * 前端：install → lint → typecheck → build（可选）
  * 后端：fmt check → clippy → build

---

## 6. 主题系统需求（可换主题 + Day/Night）

### 6.1 设计目标（具体化）

* 不把颜色/间距等设计变量散落在组件里
* 通过“主题包”切换整体视觉
* Day/Night 作为模式（mode），可叠加到主题之上

### 6.2 主题架构（建议方案）

#### 6.2.1 主题 token 层

* 使用 CSS Variables 定义 token，例如：

  * `--color-bg`
  * `--color-fg`
  * `--color-primary`
  * `--radius-md`
* 每个主题一个目录，例如：

  * `src/themes/default/`
  * `src/themes/brandA/`

#### 6.2.2 Day/Night 模式

* 使用 `data-mode="light|dark"` 或 Tailwind 的 `dark` class 模式
* Tailwind 配置 `darkMode: ['class', '[data-mode="dark"]']`（二选一）
* 默认行为：

  * 首次启动：跟随系统 `prefers-color-scheme`
  * 用户手动切换后：持久化并覆盖系统策略

#### 6.2.3 运行时切换机制（最小）

* `ThemeProvider`（React Context）维护：

  * `themeId`（例如 default/brandA）
  * `mode`（light/dark）
* 切换时更新 `document.documentElement` 的属性与加载的主题 CSS（或 class）

---

## 7. ACP + 本地 Agent 子系统需求

### 7.1 关键约束与事实（与实现强相关）

* ACP 使用 JSON-RPC 2.0，并定义了 `initialize`、`session/new`、`session/prompt` 等核心方法及 `session/update` 通知。([Agent Client Protocol][4])
* 本期采用 **stdio transport**：Client 启动 Agent 子进程，stdin/stdout 交换消息；消息以 `\n` 分隔且不得包含内嵌换行。([Agent Client Protocol][5])
* 建会话需要 `session/new`，参数包含 `cwd` 等。([Agent Client Protocol][6])

### 7.2 Agent 支持范围（模板层面的定位）

* 模板支持“**任何 ACP-compatible agent**”
* 预置示例配置面向这三类：

  * **Gemini CLI**（有 ACP 模式：`gemini --experimental-acp` 的生态使用方式较常见）([AI SDK][7])
  * **Claude Code via ACP adapter**（如 `@zed-industries/claude-code-acp`）([GitHub][8])
  * **Codex via ACP adapter**（如 `codex-acp` / `npx @zed-industries/codex-acp`）([GitHub][9])
    ACP 官方 agents 列表中也明确包含 Gemini CLI，以及 Codex CLI、Claude Code 的适配方式（via adapter）。([Agent Client Protocol][10])

> 说明：模板只负责“用 ACP 调起本地命令并对话”，不负责这些 CLI 的安装、登录与配额管理。

### 7.3 配置需求（强烈建议做成文件驱动）

#### 7.3.1 Agent 配置文件格式（建议字段）

* `id`: string（如 `gemini`, `claude`, `codex`）
* `label`: string（用于 UI 展示）
* `command`: string（可执行文件或 npx 命令）
* `args`: string[]
* `env`: Record<string,string>（可选）
* `defaultCwd`: string（可选；没有则用用户选择或应用数据目录）
* `enabled`: boolean

#### 7.3.2 工作目录（cwd）策略

* ACP 的 `session/new` 需要 `cwd`。([Agent Client Protocol][6])
* 模板提供两种最小策略：

  1. 默认 cwd：应用数据目录 / 或用户 home 下某个目录
  2. UI 提供一个“选择工作目录”的入口（文件夹选择器），并持久化

### 7.4 ACP 会话生命周期（MVP）

#### 7.4.1 初始化

* Rust backend 启动 agent 子进程后，必须先发 `initialize` 并完成版本/能力协商 ([Agent Client Protocol][2])
* 模板的 clientCapabilities：**默认不声明 fs/terminal**（保持最小闭环）

#### 7.4.2 新会话

* 调用 `session/new` 获取 `sessionId` ([Agent Client Protocol][6])
* 在 UI 层可简单处理为：每次切换 agent 就新建一个 session；或每次清空对话新建 session

#### 7.4.3 发送 prompt + 接收更新

* UI 发送 prompt → Rust 调用 `session/prompt`，prompt 内容用 `text` content block ([Agent Client Protocol][3])
* Rust 监听 agent stdout：

  * `session/update`（重点处理 `agent_message_chunk`，用于流式拼接消息）([Agent Client Protocol][3])
  * 最终 `session/prompt` response 返回 stopReason（如 `end_turn`）作为本轮结束信号 ([Agent Client Protocol][3])

---

## 8. 端到端对话 Demo（模板必须“开箱可跑”）

### 8.1 必备能力

* 前端发起 text prompt（invoke）
* Rust 透传到 ACP agent
* 处理 `session/update` 的文本 chunk
* 最终显示为一条 agent 回复（可流式更新 UI）

### 8.2 建议：内置一个“Echo ACP Agent”（保证模板开箱可用）

为避免用户没装 Gemini/Codex/Claude Code 时无法验证链路，模板建议内置一个 **Mock Agent**（例如 `acp-echo-agent`）：

* 行为：

  * `initialize`：返回最基础能力
  * `session/new`：返回固定 sessionId
  * `session/prompt`：立刻通过 `session/update` 发若干 `agent_message_chunk`（模拟流式），最后返回 stopReason `end_turn`
* 价值：

  * 模板一 clone 就能跑通 E2E
  * CI 也可以用它做“对话链路回归测试”

---

## 9. 交付物清单（模板仓库应包含）

1. **可运行的 Tauri 项目**
2. 前端集成：

   * React + Vite + Tailwind
   * i18n（至少 zh/en 两套）
   * TypeCheck + ESLint + Prettier（含 scripts）
3. 主题系统：

   * 默认主题（含 light/dark token）
   * ThemeProvider + 切换 UI
4. ACP 子系统（Rust）：

   * Agent 配置加载
   * 子进程管理（启动/退出）
   * JSON-RPC newline transport（读写）
   * `initialize` / `session/new` / `session/prompt` 实现与 `session/update` 处理
5. E2E Chat Demo 页面
6. 文档：

   * README：如何启动、如何配置 agent、如何新增主题/语言
   * `agents.example.json`：示例 agent 配置（Gemini/Claude/Codex + Echo）

---

## 10. 验收标准（可执行、可验证）

### 10.1 前端工程化

* `pnpm lint`（或 npm/yarn 等同命令）无报错
* `pnpm typecheck` 无报错
* `pnpm tauri dev` 可启动窗口并显示主界面

### 10.2 i18n

* UI 可在 zh/en 间切换，并立即生效
* 重启应用后语言设置保持

### 10.3 主题系统

* 可切换 Day/Night，并立即影响 UI（背景/文字等至少 2~3 个 token 可见变化）
* 重启应用后主题与模式保持
* 新增一个主题目录并在配置中启用后，可在 UI 中切换到新主题（不改业务组件）

### 10.4 ACP 最小对话链路

* 默认使用 Echo ACP Agent 时：

  * 输入 prompt → UI 显示 agent 流式回复 → 最终落盘为一条消息
* 更换为任意一个真实 ACP agent（用户自行安装）时：

  * 能完成 `initialize` → `session/new` → `session/prompt` 的基本交互（至少能收到文本更新或最终回复）([Agent Client Protocol][2])

---

## 11. 备注与限制（现实约束，不谈虚指标）

* **stdio 消息必须单行 JSON**（换行分隔且禁止内嵌换行），实现时必须确保 JSON 序列化不包含 `\n`。([Agent Client Protocol][5])
* 某些 agent（如 Codex/Claude）可能需要通过 **ACP adapter** 才能作为 ACP agent 被启动；这在 ACP agents 生态中是常见形态。([Agent Client Protocol][10])
* 模板不负责处理复杂 tool-call/权限体系；若未来你需要“让 agent 改文件/跑命令”，可以在此框架上继续扩展 ACP 的 fs/terminal capability 与 permission 流程（本期不做）。

---

[1]: https://agentclientprotocol.com/ "Introduction - Agent Client Protocol"
[2]: https://agentclientprotocol.com/protocol/initialization "Initialization - Agent Client Protocol"
[3]: https://agentclientprotocol.com/protocol/prompt-turn "Prompt Turn - Agent Client Protocol"
[4]: https://agentclientprotocol.com/protocol/overview "Overview - Agent Client Protocol"
[5]: https://agentclientprotocol.com/protocol/transports "Transports - Agent Client Protocol"
[6]: https://agentclientprotocol.com/protocol/session-setup "Session Setup - Agent Client Protocol"
[7]: https://ai-sdk.dev/providers/community-providers/acp?utm_source=chatgpt.com "Community Providers: ACP (Agent Client Protocol)"
[8]: https://github.com/zed-industries/claude-code-acp "GitHub - zed-industries/claude-code-acp: Use Claude Code from any ACP client such as Zed!"
[9]: https://github.com/zed-industries/codex-acp "GitHub - zed-industries/codex-acp"
[10]: https://agentclientprotocol.com/overview/agents?utm_source=chatgpt.com "Agents"
