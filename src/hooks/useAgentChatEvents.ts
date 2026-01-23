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
  type AgentStatusLike,
  type ToolCallPatch,
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
  seq?: number
  emittedAtMs?: number
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

/**
 * Extract toolCallId from a payload, checking multiple possible field names.
 */
function extractToolCallId(value: unknown): string | null {
  if (!value || typeof value !== 'object') return null

  const record = value as Record<string, unknown>
  const candidates = ['toolCallId', 'tool_call_id', 'id', 'callId']

  for (const key of candidates) {
    const val = record[key]
    if (typeof val === 'string' && val.length > 0) {
      return val
    }
  }
  return null
}

/**
 * Normalize tool call status to a displayable string.
 */
function normalizeToolCallStatus(value: unknown): string | undefined {
  if (typeof value === 'string') return value
  if (value && typeof value === 'object' && 'type' in value) {
    const type = (value as { type?: unknown }).type
    if (typeof type === 'string') return type
  }
  return undefined
}

/**
 * Parse a tool call or tool call update payload into a ToolCallPatch.
 * Returns null if toolCallId cannot be extracted.
 */
function parseToolCallPatch(payload: unknown): ToolCallPatch | null {
  const toolCallId = extractToolCallId(payload)
  if (!toolCallId) return null

  if (!payload || typeof payload !== 'object') {
    return { toolCallId }
  }

  const record = payload as Record<string, unknown>

  // Extract title (various possible field names)
  const title =
    typeof record.title === 'string'
      ? record.title
      : typeof record.name === 'string'
        ? record.name
        : undefined

  // Extract kind
  const kind = typeof record.kind === 'string' ? record.kind : undefined

  // Extract status
  const status = normalizeToolCallStatus(record.status)

  // Extract input/args
  const input = record.input ?? record.args

  // Extract result/output
  const result = record.result ?? record.output

  // Extract error
  let error: string | undefined
  if (typeof record.error === 'string') {
    error = record.error
  } else if (
    record.error &&
    typeof record.error === 'object' &&
    'message' in record.error
  ) {
    const errMsg = (record.error as { message?: unknown }).message
    if (typeof errMsg === 'string') {
      error = errMsg
    }
  }

  return {
    toolCallId,
    title,
    kind,
    status,
    input,
    result,
    error,
    raw: payload,
  }
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
      try {
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

        // If the agent stops or errors, unblock the UI and finalize any streaming message.
        // This prevents the UI from getting stuck in "sending" state if the agent process
        // dies mid-turn or encounters an unrecoverable error.
        if (status.type === 'stopped') {
          store.endAssistantStreaming(key)
          store.setSending(key, false)
          logger.debug('Agent stopped, clearing sending state', {
            workspaceId,
            agentId,
          })
        }

        if (status.type === 'errored') {
          // Note: Don't call endAssistantStreaming here - setAssistantError handles
          // clearing pendingAssistantMessageId, streaming flag, and sending state.
          // Calling endAssistantStreaming first would clear pendingAssistantMessageId,
          // causing setAssistantError to create a detached error message instead of
          // attaching the error to the streaming message.
          store.setAssistantError(key, status.message)
          logger.warn('Agent errored, error attached to assistant message', {
            workspaceId,
            agentId,
            message: status.message,
          })
        }
      } catch (error) {
        logger.error('Error processing agent/status_changed event', {
          error,
          payload: event.payload,
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
        logger.error('Failed to setup agent/status_changed listener', { error })
      })

    // Listen for ACP session updates
    listen<AcpSessionUpdateEvent>('acp/session_update', event => {
      try {
        logger.debug('ACP session update event received', {
          type: event.payload.update.type,
          workspaceId: event.payload.workspaceId,
          agentId: event.payload.agentId,
        })

        const { workspaceId, agentId, sessionId, update } = event.payload
        const store = useChatStore.getState()

        // Ensure conversation exists - events may arrive before UI has created it
        // (race between agent start / prompt ack / UI selection). Using ensureConversation
        // prevents losing early tool calls or initial message chunks.
        const key = store.ensureConversation(workspaceId, agentId)
        const conv = store.conversations[key]

        // Safety check - should never happen since ensureConversation creates it
        if (!conv) {
          logger.error('Conversation missing after ensureConversation', {
            workspaceId,
            agentId,
            key,
          })
          return
        }

        // Validate session ID - ignore updates for old sessions
        if (conv.sessionId && conv.sessionId !== sessionId) {
          logger.debug('Ignoring update for non-current session', {
            workspaceId,
            agentId,
            sessionId,
            currentSessionId: conv.sessionId,
          })
          return
        }

        // Set session ID if not yet set (in case event arrives before SendPromptAck)
        if (!conv.sessionId) {
          store.setSessionId(key, sessionId)
        }

        // Create metadata for message ordering
        const meta = {
          createdAtMs:
            typeof event.payload.emittedAtMs === 'number'
              ? event.payload.emittedAtMs
              : Date.now(),
          seq:
            typeof event.payload.seq === 'number'
              ? event.payload.seq
              : undefined,
        }

        // Handle different update types
        switch (update.type) {
          case 'userMessageChunk': {
            // Skip - we already render the user message locally
            break
          }

          case 'agentMessageChunk': {
            const text = extractChunkText(update.content)
            if (text) {
              store.appendAssistantText(key, text, meta)
            }
            break
          }

          case 'agentThoughtChunk': {
            // For now, treat thoughts as message content (could be styled differently later)
            const text = extractChunkText(update.content)
            if (text) {
              store.appendAssistantText(key, text, meta)
            }
            break
          }

          case 'toolCall': {
            // Split any streaming assistant message before tool call
            store.splitAssistantMessage(key)

            // Parse and upsert tool call, fallback to system message if no toolCallId
            const patch = parseToolCallPatch(update.toolCall)
            if (patch) {
              store.upsertToolCall(key, patch, meta)
            } else {
              // Fallback: no toolCallId, render as system message
              store.addSystemMessage(key, formatToolCall(update.toolCall), meta)
            }
            break
          }

          case 'toolCallUpdate': {
            // Split any streaming assistant message before tool call update
            store.splitAssistantMessage(key)

            // Parse and upsert tool call update, fallback to system message if no toolCallId
            const patch = parseToolCallPatch(update.toolCallUpdate)
            if (patch) {
              store.upsertToolCall(key, patch, meta)
            } else {
              // Fallback: no toolCallId, render as system message
              store.addSystemMessage(
                key,
                formatToolCallUpdate(update.toolCallUpdate),
                meta
              )
            }
            break
          }

          case 'plan': {
            store.splitAssistantMessage(key)
            store.addSystemMessage(key, formatPlan(update.plan), meta)
            break
          }

          case 'availableCommandsUpdate': {
            store.splitAssistantMessage(key)
            store.addSystemMessage(
              key,
              formatAvailableCommands(update.availableCommands),
              meta
            )
            break
          }

          case 'currentModeUpdate': {
            store.splitAssistantMessage(key)
            store.addSystemMessage(
              key,
              `Current mode: ${formatJson(update.currentModeId)}`,
              meta
            )
            break
          }

          case 'configOptionUpdate': {
            store.splitAssistantMessage(key)
            store.addSystemMessage(
              key,
              `Config options updated: ${formatJson(update.configOptions)}`,
              meta
            )
            break
          }

          case 'turnComplete': {
            store.endAssistantStreaming(key)

            // Per Zed's pattern: only tool_use is an intermediate stop reason.
            // All other stop reasons (end_turn, max_tokens, refusal, cancelled, etc.)
            // are terminal and should clear the sending state.
            // This ensures unknown stop reasons default to terminal (safe behavior)
            // rather than leaving the UI stuck in "sending" state.
            const stopReason = update.stopReason
            const stopStr =
              typeof stopReason === 'string'
                ? stopReason
                : (stopReason as { type?: string })?.type

            // Normalize to lowercase for comparison
            const normalizedStop = stopStr?.toLowerCase()

            // Only tool_use is intermediate - agent continues after tool results
            const isIntermediate = normalizedStop === 'tool_use'

            if (!isIntermediate) {
              store.setSending(key, false)
            }

            logger.debug('Turn completed', { stopReason, isIntermediate })
            break
          }

          case 'raw': {
            store.splitAssistantMessage(key)
            store.addSystemMessage(
              key,
              `Raw update: ${formatJson(update.json)}`,
              meta
            )
            break
          }

          default:
            logger.debug('Unknown ACP session update type', {
              type: (update as { type: string }).type,
            })
        }
      } catch (error) {
        logger.error('Error processing acp/session_update event', {
          error,
          payload: event.payload,
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
