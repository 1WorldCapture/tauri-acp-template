# 设计文档：Tauri + ACP 模板框架（阶段 0：先把“前端 + Tauri”架子搭起来）

> 本文聚焦 **“前端 + Tauri 的可运行骨架”**，并完成你要求的 3 个研究任务：选模板、盘点能力、列缺失与补齐设计。
> ACP / Agent 部分先**不实现**，但会在架构上 **预留接口位**，方便后续直接接入。

---

## 0. 目标与范围

### 本阶段目标（必须做到）

1. 基于一个高 Star、尽量贴近需求的 GitHub 模板，快速得到 **可运行** 的：
   - Tauri v2 + Rust 后端
   - React + Vite + Tailwind 前端

2. 前端工程能力满足你提到的框架诉求：
   - i18n（国际化）
   - 主题系统（Day/Night + 后续可扩展为多主题）
   - TypeCheck + Lint（基础工具链齐全，且可作为模板的“质量门”）-（最好有）测试与 CI 的基础能力

### 本阶段明确不做

- 不做 ACP 协议与本地 Agent CLI 调用逻辑（只预留接口位）
- 不做“性能/稳定性/可扩展性”这类虚化指标（按你的要求）

---

## 1) 研究任务：GitHub 模板选型

我调研了几个在 Tauri 生态里较常见、且 Star 相对更高的候选：

### 候选 A：agmmnn/tauri-ui（⭐ 1.8k）

- 亮点：脚手架工具 + 模板集合，主打 “快速做现代 Tauri 桌面应用”，提到 shadcn/ui、暗黑/明亮模式、GitHub Action 等。 ([GitHub][1])
- 风险/问题：它的 **最新 release 显示在 2023**，整体偏“脚手架模板集合”，是否覆盖你要的 **i18n + 测试 + 更体系化的质量门**并不明确（README 没直接强调）。([GitHub][1])
- 结论：Star 很高，但与“你要做的工程化模板框架（含 i18n/测试/质量门/主题体系）”相比，不够“确定”。

### 候选 B：MrLightful/create-tauri-react（⭐ ~190+）

- 亮点：明确是 **Vite + React + Tailwind + shadcn/ui** 的模板，且有 ESLint/Prettier/Husky 等基础工程化。 ([GitHub][2])
- 风险/问题：从 README 描述看，**测试、i18n、主题体系**不一定是“开箱即用的完整方案”（需要你自己补齐/约束）。 ([GitHub][2])
- 结论：很适合作“轻量起步”，但你想要的“模板框架能力”还要补不少。

### 候选 C（最终选择）：dannysmith/tauri-template（⭐ 157）

- 亮点：它是一个 **“batteries-included”** 的 Tauri v2 + React 19 + TS + Vite 7 模板，并且 README 明确写了：
  - UI：shadcn/ui v4 + Tailwind v4 ([GitHub][3])
  - Testing：Vitest + Testing Library ([GitHub][3])
  - Quality：ESLint、Prettier、ast-grep、knip、jscpd、clippy，并提供单一质量门 `npm run check:all` ([GitHub][3])
  - **i18n built-in（含 RTL）** ([GitHub][3])
  - **Theme System：Light/Dark + system detection + 跨窗口同步** ([GitHub][3])
  - 还有 **tauri-specta** 做 Rust ↔ TS 的 type-safe bridge ([GitHub][3])

- 结论：虽然 Star 没有 tauri-ui 那么夸张，但它是**少数在 README 明确同时覆盖你核心诉求**（Vite/React/Tailwind + i18n + 主题 + 测试 + 质量门 + Tauri v2）的模板。对“我们要做模板框架”来说，它更省迭代成本。

✅ **最终选型：`dannysmith/tauri-template` 作为我们模板框架的基底**。([GitHub][3])

---

## 2) 研究任务：选中模板能力 vs 我们的需求（匹配/不匹配/缺失）

下面只对齐你关心的“前端 + Tauri 骨架”能力点。

### 2.1 与需求高度匹配的部分（基本开箱即用）

- **React + Vite + TypeScript**：明确列为 stack。([GitHub][3])
- **Tailwind CSS**：Tailwind v4 + shadcn/ui v4。([GitHub][3])
- **i18n**：README 写明 _“i18n built-in with RTL support”_，且菜单系统也强调 i18n 支持。([GitHub][3])
- **主题系统（Day/Night）**：README 写明 _“Theme System - Light/dark mode with system preference detection, synced across windows”_。([GitHub][3])
- **TypeCheck & Lint & 质量门**：
  - 工具：ESLint、Prettier、ast-grep、knip、jscpd、clippy
  - 统一入口：`npm run check:all` ([GitHub][3])

- **测试**：Vitest + Testing Library，并提到 “Tauri command mocking”。([GitHub][3])

> 结论：就你列的前端工程诉求而言，这个模板基本都覆盖了。

---

### 2.2 不匹配（“超出我们模板框架目标”，建议裁剪/降级为可选）

这个模板“很豪华”，但你当前阶段只要“框架”，以下属于 **功能超配**：

- Command Palette（Cmd+K）([GitHub][3])
- Quick Pane / 多窗口 + 全局快捷键示例([GitHub][3])
- Auto-updates、Notifications、Crash Recovery、Logging 等应用级能力([GitHub][3])

这些不是坏东西，但会：

- 增加你后续接入 ACP 时的认知负担
- 让模板“看起来像一个完整 App”，而不是“干净的框架底座”

✅ **建议**：我们在派生模板中把它们做成 **可选 Feature（默认关闭/删除）**，保留“能力接口与最小示例”。

---

### 2.3 真正“缺失/需要我们补齐”的点（相对你更长线的主题诉求）

严格按你写的要求：“**基于主题的架构，方便后续更换主题，并支持 Day/Night**”。

模板已支持 Day/Night，但通常这类模板的“主题”可能仅止于：

- Light / Dark 两套 token（或一套 token + dark 覆盖）

你提到的“方便后续更换主题”更像是：

- **多套主题包**（例如：Default / Slate / Indigo / Rose…）
- 每套主题同时支持 Day/Night（或者主题色 + 模式分离）

因此，这里我认为**唯一需要我们补齐的核心缺失项**是：

- **多主题架构（Theme Packs）**：在现有 light/dark 的基础上，引入 `themeName`（主题名）概念，并提供主题扩展机制（不只是 dark mode）。

  > 这是“模板框架”层面最关键的补强点。

---

## 3) 研究任务：缺失项的设计（如何在该模板基础上弥补）

下面给出一个“尽量不推翻模板现有实现”的补齐方案：**在现有 Theme System 上扩展为“主题名 + 模式”双维度**。

### 3.1 目标形态

- `mode`: `"system" | "light" | "dark"`
- `theme`: `"default" | "slate" | "indigo" | ..."`（可扩展）
- DOM 体现：
  - `document.documentElement.classList` 控制 `dark`（与 Tailwind/shadcn 兼容）
  - `document.documentElement.dataset.theme = themeName` 控制多主题 token

### 3.2 CSS Token 组织建议（兼容 shadcn/tailwind 的思路）

新增（或整理）：

```
src/styles/themes/
  tokens.css                # 定义 token 变量名（语义化）
  theme-default.css         # [data-theme="default"] 下的变量值
  theme-slate.css           # [data-theme="slate"] 下的变量值
  ...
  mode-dark-overrides.css   # 可选：dark 模式的覆盖（也可以写在每个 theme 文件里）
```

示意（伪代码）：

```css
/* theme-default.css */
:root[data-theme='default'] {
  --bg: 0 0% 100%;
  --fg: 240 10% 3.9%;
  /* ... */
}

:root[data-theme='default'].dark {
  --bg: 240 10% 3.9%;
  --fg: 0 0% 98%;
}
```

这样你以后新增主题，只要新加一个 `theme-xxx.css`，无需改组件代码。

### 3.3 ThemeProvider（React）组织建议

在模板已有 Theme System 基础上，抽象出统一入口（如果模板已有 provider，就扩展）：

- `src/app/providers/theme-provider.tsx`
- 提供：
  - `useTheme()`：读取/设置 `{ theme, mode }`
  - 持久化：localStorage（模板如果已有 Rust preferences persistence，可直接复用它的 setting 系统）([GitHub][3])
  - system mode：监听 `prefers-color-scheme`

### 3.4 i18n：我们需要做什么？

模板已经“built-in”，但作为你自己的框架模板，建议补一层**约束**，让未来迭代可控：

- 统一约定：`locales/<lang>/<namespace>.json`（模板中已存在 `locales` 目录）([GitHub][3])
- 提供最小示例：
  - `locales/en/common.json`
  - `locales/zh-CN/common.json`

- 提供一个“语言切换”UI（放到 Preferences/Settings 页面里）
- 约束点：
  - 默认语言策略：system → fallback en
  - 是否允许 RTL：模板已提 RTL 支持，你可保留开关或做自动检测 ([GitHub][3])

> 这不是“补齐缺失”，而是把现有能力“模板化”：让未来所有项目都沿同一规范走。

---

## 4) 先把“前端 + Tauri”架子搭起来：落地步骤

这里给你一套 **可以直接执行** 的起步流程（完全按模板 README 的 Quick Start）。

### 4.1 初始化项目（从 GitHub Template 开始）

方式 1（推荐）：GitHub 上直接 “Use this template”

- 选择 `dannysmith/tauri-template` 作为模板创建你自己的仓库 ([GitHub][3])

方式 2：命令行（你自己创建 repo 后 clone）

```bash
git clone <your-repo>
cd your-app
npm install
npm run dev
```

模板 README 给出的 Quick Start 就是上面这套。([GitHub][3])

> 前置依赖：Node.js 18+、Rust stable，以及各平台 Tauri 依赖（README 指向 tauri.app 的 prerequisites）。([GitHub][3])

### 4.2 验收标准（“架子搭好”的定义）

你本地跑起来后，满足：

- `npm run dev` 能拉起 Tauri 窗口 ([GitHub][3])
- 你能看到模板自带 UI（不关心具体长啥，关心“跑起来”）
- `npm run check:all` 能跑通（模板定义的统一质量门）([GitHub][3])

到这里，“前端 + Tauri”骨架就成立了。

---

## 5) 在该模板之上，我们建议的“派生模板改造”方案

为了把它从“豪华 App”变成“ACP 框架底座模板”，建议按 2 条主线改造：

### 5.1 主线 A：裁剪到“框架最小集”

**保留：**

- React/Vite/Tailwind/shadcn 基础
- i18n、Theme System
- tests、lint、typecheck、`check:all`
- tauri-specta（以后 ACP command 会很香）([GitHub][3])

**默认移除/关闭（可作为后续可选 feature 再加回来）：**

- Command Palette
- Quick Pane / 多窗口 demo
- Auto-updates（除非你希望模板默认带发布能力）
- Crash recovery / notifications 等应用级功能 ([GitHub][3])

> 这一步的产物是：一个“干净的桌面应用壳”，只保留框架能力与最少示例。

### 5.2 主线 B：把 Theme System 升级为“多主题 + Day/Night”

- 引入 `themeName` 维度（default/slate/…）
- 统一 token 文件组织
- Settings 页提供 Theme 与 Mode 的切换 UI
- 持久化沿用模板的 preferences 系统（模板已有“Preferences System - Rust-side persistence”）([GitHub][3])

---

## 6) 你接下来可以怎么迭代（建议按 PR 拆分）

为了符合“从框架开始迭代”的节奏，我建议你把第一阶段拆成 3 个 PR：

### PR-0：Bootstrap（不改逻辑）

- 从 `dannysmith/tauri-template` 创建仓库
- 改项目名 / app identifier（如果需要）
- 确保 `npm run dev` 跑起来 ([GitHub][3])

### PR-1：瘦身（裁剪成框架模板）

- 移除/隐藏 command palette、quick pane、多窗口 demo
- 保留 i18n/theme/tests/lint/typecheck 体系
- 确保 `check:all` 仍能跑通 ([GitHub][3])

### PR-2：主题系统升级（多主题）

- data-theme + dark class 双机制
- 增加 2 套主题样例（default + slate）
- Settings UI 接入 theme/mode

---

## 7) 额外说明：为什么这个模板对后续 ACP 很友好

虽然本阶段不做 ACP，但这个模板提供了两点“未来会省很多事”的基础设施：

- **Type-safe Rust ↔ TS**：tauri-specta（将来我们定义 ACP 调用 command 时非常合适）([GitHub][3])
- **测试支持 Tauri command mocking**：README 明确提到 testing patterns（Vitest + mocking），而官方 Tauri 文档也有 mockIPC 思路（你后续做 ACP/Agent 的单测会用到）。([GitHub][3])

---

[1]: https://github.com/agmmnn/tauri-ui 'https://github.com/agmmnn/tauri-ui'
[2]: https://github.com/MrLightful/create-tauri-react 'https://github.com/MrLightful/create-tauri-react'
[3]: https://github.com/dannysmith/tauri-template 'GitHub - dannysmith/tauri-template: A production-ready template for building modern desktop applications with Tauri v2, React 19, and TypeScript. This template provides a solid foundation with best practices, comprehensive documentation, and quality tooling built-in.'
