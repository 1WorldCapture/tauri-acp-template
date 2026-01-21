# AI Agent Instructions

## Overview

This repository is a Tauri v2 + React + Rust desktop application template with:
- **ACP Integration**: Agent Client Protocol for AI agent connectivity
- **Multi-Theme System**: 6 pre-built color themes with dark/light mode support
- **Type-Safe IPC**: Rust-to-TypeScript bindings via tauri-specta

## Quick Start (New Sessions)

- Read `docs/tasks.md` for task management
- Review `docs/developer/architecture-guide.md` for high-level patterns
- Check `docs/developer/README.md` for the full documentation index
- Review `docs/acp/` for ACP protocol integration details
- Check git status and project structure

## Core Development Rules

**CRITICAL:** Follow these strictly:

0. **Use npm only**: This project uses `npm`, NOT `pnpm`. Always use `npm install`, `npm run`, etc.
1. **Read Before Editing**: Always read files first to understand context
2. **Follow Established Patterns**: Use patterns from this file and `docs/developer`
3. **Senior Architect Mindset**: Consider performance, maintainability, testability
4. **Batch Operations**: Use multiple tool calls in single responses
5. **Match Code Style**: Follow existing formatting and patterns
6. **Test Coverage**: Write comprehensive tests for business logic
7. **Quality Gates**: Run `npm run check:all` after significant changes
8. **No Dev Server**: Ask user to run and report back
9. **No Unsolicited Commits**: Only when explicitly requested
10. **Documentation**: Update relevant `docs/developer/` files for new patterns
11. **Removing files**: Always use `rm -f`

**CRITICAL:** Use Tauri v2 docs only. Always use modern Rust formatting: `format!("{variable}")`

**Version Requirements**: Tauri v2.x, shadcn/ui v4.x, Tailwind v4.x, React 19.x, Zustand v5.x, Vite v7.x, Vitest v4.x

## Architecture Patterns (CRITICAL)

### State Management Onion

```
useState (component) → Zustand (global UI) → TanStack Query (persistent data)
```

**Decision**: Is data needed across components? → Does it persist between sessions?

### Performance Pattern (CRITICAL)

```typescript
// ✅ GOOD: Selector syntax - only re-renders when specific value changes
const leftSidebarVisible = useUIStore(state => state.leftSidebarVisible)

// ❌ BAD: Destructuring causes render cascades (caught by ast-grep)
const { leftSidebarVisible } = useUIStore()

// ✅ GOOD: Use getState() in callbacks for current state
const handleAction = () => {
  const { data, setData } = useStore.getState()
  setData(newData)
}
```

### Static Analysis

- **React Compiler**: Handles memoization automatically - no manual `useMemo`/`useCallback` needed
- **ast-grep**: Enforces architecture patterns (e.g., no Zustand destructuring). See `docs/developer/static-analysis.md`
- **Knip/jscpd**: Periodic cleanup tools for dead code and duplication detection

### Event-Driven Bridge

- **Rust → React**: `app.emit("event-name", data)` → `listen("event-name", handler)`
- **React → Rust**: Use typed commands from `@/lib/tauri-bindings` (tauri-specta)
- **Commands**: All actions flow through centralized command system

### Tauri Command Pattern (tauri-specta)

```typescript
// ✅ GOOD: Type-safe commands with Result handling
import { commands } from '@/lib/tauri-bindings'

const result = await commands.loadPreferences()
if (result.status === 'ok') {
  console.log(result.data.theme)
}

// ❌ BAD: String-based invoke (no type safety)
const prefs = await invoke('load_preferences')
```

**Adding commands**: See `docs/developer/tauri-commands.md`

### Internationalization (i18n)

```typescript
// ✅ GOOD: Use useTranslation hook in React components
import { useTranslation } from 'react-i18next'

function MyComponent() {
  const { t } = useTranslation()
  return <h1>{t('myFeature.title')}</h1>
}

// ✅ GOOD: Non-React contexts - bind for many calls, or use directly
import i18n from '@/i18n/config'
const t = i18n.t.bind(i18n)  // Bind once for many translations
i18n.t('key')                 // Or call directly for occasional use
```

- **Translations**: All strings in `/locales/*.json`
- **RTL Support**: Use CSS logical properties (`text-start` not `text-left`)
- **Adding strings**: See `docs/developer/i18n-patterns.md`

## ACP Integration Overview

Agent Client Protocol (ACP) enables communication with AI agent adapters (Claude Code, Codex, Gemini CLI) via STDIO JSON-RPC.

### Module Namespace

The Rust backend uses a three-layer architecture to separate concerns:

- **`runtime/*`** - Product domain (Workspace, Agent, Operation, Permission, Terminal, FS)
- **`protocols/*`** - Protocol abstraction (`AgentConnection` trait + `AgentHost` callbacks)
  - `protocols/acp/*` - ACP protocol implementation
- **`plugins/*`** - Plugin/adapter management (installation, status, updates)

### Key Concepts

| Concept | Description |
|---------|-------------|
| `workspaceId` | Workspace isolation identifier (UUID) |
| `operationId` | Atomic operation tracking (UUID) |
| `agentId` | Agent instance within workspace |
| `sessionId` | ACP session identifier |

### Permission System

Every capability operation (terminal execution, file read/write, adapter installation) requires explicit user permission. No automatic policies - the app framework provides mechanisms, user decisions drive authorization.

**Detailed documentation**: `docs/acp/low-level-design.md`

## Multi-Theme System

6 pre-built color themes using CSS custom properties and OKLCH color space:

| Theme | Class |
|-------|-------|
| Default | (none) |
| Claude | `theme-claude` |
| Perplexity | `theme-perplexity` |
| Cosmic Night | `theme-cosmic-night` |
| Modern Minimal | `theme-modern-minimal` |
| Ocean Breeze | `theme-ocean-breeze` |

- **Configuration**: `src/lib/theme-config.ts`
- **Theme CSS**: `src/themes/*.css`
- **Mode support**: Each theme supports both light and dark modes

## Developer Documentation

For complete patterns and detailed guidance, see `docs/developer/README.md`.

### Core Documents

- `architecture-guide.md` - Mental models, security, anti-patterns
- `state-management.md` - State onion, getState() pattern details
- `tauri-commands.md` - Adding new Rust commands
- `static-analysis.md` - All linting tools and quality gates

### ACP Documents

- `docs/acp/technical-requirements.md` - System requirements and constraints
- `docs/acp/design-phase-1.md` - Phase 1 architecture design
- `docs/acp/low-level-design.md` - User story-level implementation details
