/**
 * Agent chat event listener hook for ACP session updates and agent status events.
 *
 * Listens for:
 * - `agent/status_changed` - When agent runtime status changes (starting, running, errored)
 * - `acp/session_update` - When agent sends message chunks, tool calls, etc.
 */

import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'
import { logger } from '@/lib/logger'
import {
  useChatStore,
  makeChatKeyFromIds,
  type AgentStatusLike,
} from '@/store/chat-store'

// ============================================================================
// Event Payload Types (matching Rust types in api/types.rs)
// ============================================================================

/**
 * Agent runtime status.
 * Matches Rust AgentRuntimeStatus enum.
 */
type AgentRuntimeStatus =
  | { type: 'stopped' }
  | { type: 'starting' }
  | { type: 'running'; sessionId: string }
  | { type: 'errored'; message: string }

/**
 * Agent status changed event payload.
 * Matches Rust AgentStatusChangedEvent.
 */
interface AgentStatusChangedEvent {
  workspaceId: string
  agentId: string
  status: AgentRuntimeStatus
}

/**
 * ACP session update types.
 * Matches Rust AcpSessionUpdate enum.
 */
type AcpSessionUpdate =
  | { type: 'userMessageChunk'; content: unknown }
  | { type: 'agentMessageChunk'; content: unknown }
  | { type: 'agentThoughtChunk'; content: unknown }
  | { type: 'toolCall'; toolCall: unknown }
  | { type: 'toolCallUpdate'; toolCallUpdate: unknown }
  | { type: 'plan'; plan: unknown }
  | { type: 'availableCommandsUpdate'; availableCommands: unknown }
  | { type: 'currentModeUpdate'; currentModeId: unknown }
  | { type: 'configOptionUpdate'; configOptions: unknown }
  | { type: 'turnComplete'; stopReason: unknown }
  | { type: 'raw'; json: unknown }

/**
 * ACP session update event payload.
 * Matches Rust AcpSessionUpdateEvent.
 */
interface AcpSessionUpdateEvent {
  workspaceId: string
  agentId: string
  sessionId: string
  update: AcpSessionUpdate
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Extract text content from an ACP message chunk.
 * Content can be:
 * - A string directly
 * - An object with a `text` field: { text: "..." }
 * - An array of content blocks: [{ type: "text", text: "..." }, ...]
 */
function extractChunkText(content: unknown): string {
  if (typeof content === 'string') {
    return content
  }

  // Handle array of content blocks
  if (Array.isArray(content)) {
    return content
      .map(block => {
        if (typeof block === 'string') {
          return block
        }
        if (block && typeof block === 'object' && 'text' in block) {
          return String((block as { text: unknown }).text)
        }
        return ''
      })
      .filter(Boolean)
      .join('')
  }

  // Handle object with text field
  if (
    content &&
    typeof content === 'object' &&
    'text' in content &&
    typeof (content as { text: unknown }).text === 'string'
  ) {
    return (content as { text: string }).text
  }

  return ''
}

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

function formatToolCall(toolCall: unknown): string {
  if (!toolCall || typeof toolCall !== 'object') {
    return `Tool call: ${formatJson(toolCall)}`
  }

  const record = toolCall as Record<string, unknown>
  const title = typeof record.title === 'string' ? record.title : undefined
  const kind = typeof record.kind === 'string' ? record.kind : undefined
  const status = typeof record.status === 'string' ? record.status : undefined
  const id =
    typeof record.toolCallId === 'string' ? record.toolCallId : undefined

  const header = title ? `Tool call: ${title}` : 'Tool call'
  const meta = [
    kind && `kind=${kind}`,
    status && `status=${status}`,
    id && `id=${id}`,
  ]
    .filter(Boolean)
    .join(' • ')

  if (meta) {
    return `${header}\n${meta}`
  }
  return header
}

function formatToolCallUpdate(toolCallUpdate: unknown): string {
  if (!toolCallUpdate || typeof toolCallUpdate !== 'object') {
    return `Tool update: ${formatJson(toolCallUpdate)}`
  }

  const record = toolCallUpdate as Record<string, unknown>
  const id =
    typeof record.toolCallId === 'string' ? record.toolCallId : undefined
  const status = typeof record.status === 'string' ? record.status : undefined

  const header = 'Tool update'
  const meta = [status && `status=${status}`, id && `id=${id}`]
    .filter(Boolean)
    .join(' • ')

  if (meta) {
    return `${header}\n${meta}`
  }
  return header
}

function formatPlan(plan: unknown): string {
  if (!plan || typeof plan !== 'object') {
    return `Plan update: ${formatJson(plan)}`
  }

  const record = plan as Record<string, unknown>
  const entries = Array.isArray(record.entries) ? record.entries : []
  if (entries.length === 0) {
    return 'Plan update: (empty)'
  }

  const lines = entries.map((entry, index) => {
    if (!entry || typeof entry !== 'object') {
      return `${index + 1}. ${formatJson(entry)}`
    }
    const entryRecord = entry as Record<string, unknown>
    const content =
      typeof entryRecord.content === 'string'
        ? entryRecord.content
        : formatJson(entryRecord.content)
    const status =
      typeof entryRecord.status === 'string' ? entryRecord.status : undefined
    const priority =
      typeof entryRecord.priority === 'string'
        ? entryRecord.priority
        : undefined
    const meta = [
      status && `status=${status}`,
      priority && `priority=${priority}`,
    ]
      .filter(Boolean)
      .join(' • ')
    return meta
      ? `${index + 1}. ${content} (${meta})`
      : `${index + 1}. ${content}`
  })

  return ['Plan:', ...lines].join('\n')
}

function formatAvailableCommands(availableCommands: unknown): string {
  if (!Array.isArray(availableCommands)) {
    return `Available commands update: ${formatJson(availableCommands)}`
  }

  const names = availableCommands
    .map(command => {
      if (command && typeof command === 'object' && 'name' in command) {
        const name = (command as { name?: unknown }).name
        return typeof name === 'string' ? name : null
      }
      return null
    })
    .filter(Boolean)
    .join(', ')

  if (!names) {
    return 'Available commands update: (no names)'
  }

  return `Available commands updated (${availableCommands.length}): ${names}`
}

// ============================================================================
// Hook
// ============================================================================

/**
 * Hook to listen for agent chat events from the backend.
 *
 * Updates the chat store with:
 * - Agent status changes (starting, running, errored)
 * - Message chunks from the agent (streaming responses)
 *
 * Should be mounted once at the app level (e.g., in MainWindow).
 */
export function useAgentChatEvents() {
  useEffect(() => {
    let isMounted = true
    const unlisteners: (() => void)[] = []

    // Listen for agent status changes
    listen<AgentStatusChangedEvent>('agent/status_changed', event => {
      logger.debug('Agent status changed event received', {
        payload: event.payload,
      })

      const { workspaceId, agentId, status } = event.payload
      const store = useChatStore.getState()

      // Ensure conversation exists and get its key
      const key = store.ensureConversation(workspaceId, agentId)

      // Map Rust status to our store type
      const statusLike: AgentStatusLike = status

      store.setAgentStatus(key, statusLike)
    })
      .then(unlisten => {
        if (!isMounted) {
          unlisten()
        } else {
          unlisteners.push(unlisten)
        }
      })
      .catch(error => {
        logger.error('Failed to setup agent/status_changed listener', { error })
      })

    // Listen for ACP session updates
    listen<AcpSessionUpdateEvent>('acp/session_update', event => {
      logger.debug('ACP session update event received', {
        type: event.payload.update.type,
        workspaceId: event.payload.workspaceId,
        agentId: event.payload.agentId,
      })

      const { workspaceId, agentId, update } = event.payload
      const store = useChatStore.getState()

      // Get the key for this conversation
      const key = makeChatKeyFromIds(workspaceId, agentId)

      // Check if conversation exists (it should, since agent should be started)
      const conv = store.conversations[key]
      if (!conv) {
        logger.warn('Received session update for unknown conversation', {
          workspaceId,
          agentId,
        })
        return
      }

      // Handle different update types
      switch (update.type) {
        case 'userMessageChunk': {
          const text = extractChunkText(update.content)
          if (text) {
            store.addSystemMessage(key, `User message: ${text}`)
          }
          break
        }

        case 'agentMessageChunk': {
          const text = extractChunkText(update.content)
          if (text) {
            store.appendAssistantText(key, text)
          }
          break
        }

        case 'agentThoughtChunk': {
          // For now, treat thoughts as message content (could be styled differently later)
          const text = extractChunkText(update.content)
          if (text) {
            store.appendAssistantText(key, text)
          }
          break
        }

        case 'toolCall': {
          store.addSystemMessage(key, formatToolCall(update.toolCall))
          break
        }

        case 'toolCallUpdate': {
          store.addSystemMessage(
            key,
            formatToolCallUpdate(update.toolCallUpdate)
          )
          break
        }

        case 'plan': {
          store.addSystemMessage(key, formatPlan(update.plan))
          break
        }

        case 'availableCommandsUpdate': {
          store.addSystemMessage(
            key,
            formatAvailableCommands(update.availableCommands)
          )
          break
        }

        case 'currentModeUpdate': {
          store.addSystemMessage(
            key,
            `Current mode: ${formatJson(update.currentModeId)}`
          )
          break
        }

        case 'configOptionUpdate': {
          store.addSystemMessage(
            key,
            `Config options updated: ${formatJson(update.configOptions)}`
          )
          break
        }

        case 'turnComplete': {
          store.endAssistantStreaming(key)
          store.setSending(key, false)
          logger.debug('Turn completed', { stopReason: update.stopReason })
          break
        }

        case 'raw': {
          store.addSystemMessage(key, `Raw update: ${formatJson(update.json)}`)
          break
        }

        default:
          logger.debug('Unknown ACP session update type', {
            type: (update as { type: string }).type,
          })
      }
    })
      .then(unlisten => {
        if (!isMounted) {
          unlisten()
        } else {
          unlisteners.push(unlisten)
        }
      })
      .catch(error => {
        logger.error('Failed to setup acp/session_update listener', { error })
      })

    return () => {
      isMounted = false
      unlisteners.forEach(unlisten => unlisten())
    }
  }, [])
}
