import { create } from 'zustand'
import { devtools } from 'zustand/middleware'

// ============================================================================
// Types
// ============================================================================

export type ChatRole = 'user' | 'assistant' | 'system'

export interface ChatMessage {
  id: string
  role: ChatRole
  text: string
  createdAtMs: number
  seq?: number
  streaming?: boolean
  error?: string
}

// ============================================================================
// Tool Call Types
// ============================================================================

export type ToolCallStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | string

export interface ToolCallEntry {
  toolCallId: string
  title?: string
  kind?: string
  status?: ToolCallStatus
  input?: unknown
  result?: unknown
  error?: string
  createdAtMs: number
  seq?: number
  updatedAtMs: number
  raw?: unknown
}

export type ToolCallPatch = Partial<
  Omit<ToolCallEntry, 'toolCallId' | 'createdAtMs' | 'seq'>
> & {
  toolCallId: string
}

/** Optional metadata for message creation (for ordering from backend events) */
interface MessageMeta {
  createdAtMs?: number
  seq?: number
}

export type AgentStatusLike =
  | { type: 'stopped' }
  | { type: 'starting' }
  | { type: 'running'; sessionId: string }
  | { type: 'errored'; message: string }

export interface ChatConversation {
  workspaceId: string
  agentId: string
  sessionId: string | null
  agentStatus: AgentStatusLike | null
  messages: ChatMessage[]
  toolCalls: Record<string, ToolCallEntry>
  pendingAssistantMessageId: string | null
  sending: boolean
}

/** Composite key for per-agent conversations */
export type ChatKey = `${string}:${string}`

function makeChatKey(workspaceId: string, agentId: string): ChatKey {
  return `${workspaceId}:${agentId}`
}

function generateId(): string {
  return `${Date.now()}-${Math.random().toString(36).slice(2, 11)}`
}

// ============================================================================
// Store
// ============================================================================

interface ChatState {
  conversations: Record<ChatKey, ChatConversation>

  // Ensure a conversation exists for the given workspace/agent, returning the key
  ensureConversation: (workspaceId: string, agentId: string) => ChatKey

  // Reset a conversation to empty state (called when agent is selected)
  resetConversation: (key: ChatKey) => void

  // Update agent runtime status
  setAgentStatus: (key: ChatKey, status: AgentStatusLike) => void

  // Set session ID (usually after send prompt ack)
  setSessionId: (key: ChatKey, sessionId: string) => void

  // Add a user message, returns the message ID
  addUserMessage: (key: ChatKey, text: string, meta?: MessageMeta) => string

  // Add a system message, returns the message ID
  addSystemMessage: (key: ChatKey, text: string, meta?: MessageMeta) => string

  // Begin an assistant message (streaming), returns the message ID
  beginAssistantMessage: (key: ChatKey) => string

  // Append text to the pending assistant message
  appendAssistantText: (key: ChatKey, chunk: string, meta?: MessageMeta) => void

  // Split/complete the current assistant message segment (for timeline ordering)
  splitAssistantMessage: (key: ChatKey) => void

  // Mark the pending assistant message as complete (not streaming)
  endAssistantStreaming: (key: ChatKey) => void

  // Set the sending state
  setSending: (key: ChatKey, sending: boolean) => void

  // Set error on the pending assistant message
  setAssistantError: (key: ChatKey, message: string) => void

  // Upsert a tool call entry (insert or update by toolCallId)
  upsertToolCall: (
    key: ChatKey,
    patch: ToolCallPatch,
    meta?: MessageMeta
  ) => void
}

function createEmptyConversation(
  workspaceId: string,
  agentId: string
): ChatConversation {
  return {
    workspaceId,
    agentId,
    sessionId: null,
    agentStatus: null,
    messages: [],
    toolCalls: {},
    pendingAssistantMessageId: null,
    sending: false,
  }
}

export const useChatStore = create<ChatState>()(
  devtools(
    (set, get) => ({
      conversations: {},

      ensureConversation: (workspaceId, agentId) => {
        const key = makeChatKey(workspaceId, agentId)
        const existing = get().conversations[key]
        if (!existing) {
          set(
            state => ({
              conversations: {
                ...state.conversations,
                [key]: createEmptyConversation(workspaceId, agentId),
              },
            }),
            undefined,
            'ensureConversation'
          )
        }
        return key
      },

      resetConversation: key => {
        const existing = get().conversations[key]
        if (existing) {
          set(
            state => ({
              conversations: {
                ...state.conversations,
                [key]: createEmptyConversation(
                  existing.workspaceId,
                  existing.agentId
                ),
              },
            }),
            undefined,
            'resetConversation'
          )
        }
      },

      setAgentStatus: (key, status) => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state
            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  agentStatus: status,
                  // Also set sessionId if status is running
                  sessionId:
                    status.type === 'running'
                      ? status.sessionId
                      : conv.sessionId,
                },
              },
            }
          },
          undefined,
          'setAgentStatus'
        )
      },

      setSessionId: (key, sessionId) => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state
            return {
              conversations: {
                ...state.conversations,
                [key]: { ...conv, sessionId },
              },
            }
          },
          undefined,
          'setSessionId'
        )
      },

      addUserMessage: (key, text, meta) => {
        const id = generateId()
        const createdAtMs = meta?.createdAtMs ?? Date.now()
        const seq = meta?.seq

        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state
            const message: ChatMessage = {
              id,
              role: 'user',
              text,
              createdAtMs,
              ...(typeof seq === 'number' ? { seq } : {}),
            }
            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  messages: [...conv.messages, message],
                },
              },
            }
          },
          undefined,
          'addUserMessage'
        )
        return id
      },

      addSystemMessage: (key, text, meta) => {
        const id = generateId()
        const createdAtMs = meta?.createdAtMs ?? Date.now()
        const seq = meta?.seq

        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state
            const message: ChatMessage = {
              id,
              role: 'system',
              text,
              createdAtMs,
              ...(typeof seq === 'number' ? { seq } : {}),
            }
            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  messages: [...conv.messages, message],
                },
              },
            }
          },
          undefined,
          'addSystemMessage'
        )
        return id
      },

      beginAssistantMessage: key => {
        const id = generateId()
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state
            const message: ChatMessage = {
              id,
              role: 'assistant',
              text: '',
              createdAtMs: Date.now(),
              streaming: true,
            }
            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  messages: [...conv.messages, message],
                  pendingAssistantMessageId: id,
                },
              },
            }
          },
          undefined,
          'beginAssistantMessage'
        )
        return id
      },

      appendAssistantText: (key, chunk, meta) => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state

            const createdAtMs = meta?.createdAtMs ?? Date.now()
            const seq = meta?.seq

            let pendingId = conv.pendingAssistantMessageId

            // If no pending message, create one
            if (!pendingId) {
              pendingId = generateId()
              const newMessage: ChatMessage = {
                id: pendingId,
                role: 'assistant',
                text: chunk,
                createdAtMs,
                streaming: true,
                ...(typeof seq === 'number' ? { seq } : {}),
              }
              return {
                conversations: {
                  ...state.conversations,
                  [key]: {
                    ...conv,
                    messages: [...conv.messages, newMessage],
                    pendingAssistantMessageId: pendingId,
                  },
                },
              }
            }

            // Append to existing pending message
            const updatedMessages = conv.messages.map(msg =>
              msg.id === pendingId ? { ...msg, text: msg.text + chunk } : msg
            )
            return {
              conversations: {
                ...state.conversations,
                [key]: { ...conv, messages: updatedMessages },
              },
            }
          },
          undefined,
          'appendAssistantText'
        )
      },

      splitAssistantMessage: key => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv || !conv.pendingAssistantMessageId) return state

            const pendingId = conv.pendingAssistantMessageId
            const pending = conv.messages.find(m => m.id === pendingId)

            if (!pending) {
              return {
                conversations: {
                  ...state.conversations,
                  [key]: { ...conv, pendingAssistantMessageId: null },
                },
              }
            }

            // If it's an empty placeholder, drop it entirely
            if (!pending.text) {
              return {
                conversations: {
                  ...state.conversations,
                  [key]: {
                    ...conv,
                    messages: conv.messages.filter(m => m.id !== pendingId),
                    pendingAssistantMessageId: null,
                  },
                },
              }
            }

            // Mark as complete (not streaming) and clear pending ID
            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  messages: conv.messages.map(m =>
                    m.id === pendingId ? { ...m, streaming: false } : m
                  ),
                  pendingAssistantMessageId: null,
                },
              },
            }
          },
          undefined,
          'splitAssistantMessage'
        )
      },

      endAssistantStreaming: key => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv || !conv.pendingAssistantMessageId) return state

            const updatedMessages = conv.messages.map(msg =>
              msg.id === conv.pendingAssistantMessageId
                ? { ...msg, streaming: false }
                : msg
            )
            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  messages: updatedMessages,
                  pendingAssistantMessageId: null,
                },
              },
            }
          },
          undefined,
          'endAssistantStreaming'
        )
      },

      setSending: (key, sending) => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state
            return {
              conversations: {
                ...state.conversations,
                [key]: { ...conv, sending },
              },
            }
          },
          undefined,
          'setSending'
        )
      },

      setAssistantError: (key, message) => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state

            // If we have a pending assistant message, attach the error to it
            if (conv.pendingAssistantMessageId) {
              const updatedMessages = conv.messages.map(msg =>
                msg.id === conv.pendingAssistantMessageId
                  ? { ...msg, error: message, streaming: false }
                  : msg
              )
              return {
                conversations: {
                  ...state.conversations,
                  [key]: {
                    ...conv,
                    messages: updatedMessages,
                    pendingAssistantMessageId: null,
                    sending: false,
                  },
                },
              }
            }

            // Otherwise create a new assistant error message
            const id = generateId()
            const errorMsg: ChatMessage = {
              id,
              role: 'assistant',
              text: '',
              createdAtMs: Date.now(),
              streaming: false,
              error: message,
            }

            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  messages: [...conv.messages, errorMsg],
                  sending: false,
                },
              },
            }
          },
          undefined,
          'setAssistantError'
        )
      },

      upsertToolCall: (key, patch, meta) => {
        set(
          state => {
            const conv = state.conversations[key]
            if (!conv) return state

            const now = meta?.createdAtMs ?? Date.now()
            const seq = meta?.seq
            const existing = conv.toolCalls[patch.toolCallId]

            let updatedEntry: ToolCallEntry

            if (existing) {
              // Update existing: preserve createdAtMs/seq, update other fields
              updatedEntry = {
                ...existing,
                updatedAtMs: now,
                // Only apply defined fields from patch (avoid overwriting with undefined)
                ...(patch.title !== undefined && { title: patch.title }),
                ...(patch.kind !== undefined && { kind: patch.kind }),
                ...(patch.status !== undefined && { status: patch.status }),
                ...(patch.input !== undefined && { input: patch.input }),
                ...(patch.result !== undefined && { result: patch.result }),
                ...(patch.error !== undefined && { error: patch.error }),
                ...(patch.raw !== undefined && { raw: patch.raw }),
              }
            } else {
              // Create new entry
              updatedEntry = {
                toolCallId: patch.toolCallId,
                title: patch.title,
                kind: patch.kind,
                status: patch.status,
                input: patch.input,
                result: patch.result,
                error: patch.error,
                raw: patch.raw,
                createdAtMs: now,
                seq: typeof seq === 'number' ? seq : undefined,
                updatedAtMs: now,
              }
            }

            return {
              conversations: {
                ...state.conversations,
                [key]: {
                  ...conv,
                  toolCalls: {
                    ...conv.toolCalls,
                    [patch.toolCallId]: updatedEntry,
                  },
                },
              },
            }
          },
          undefined,
          'upsertToolCall'
        )
      },
    }),
    { name: 'chat-store' }
  )
)

// ============================================================================
// Selectors
// ============================================================================

export function selectConversation(
  workspaceId: string | null,
  agentId: string | null
): (state: ChatState) => ChatConversation | null {
  return state => {
    if (!workspaceId || !agentId) return null
    const key = makeChatKey(workspaceId, agentId)
    return state.conversations[key] ?? null
  }
}

export function makeChatKeyFromIds(
  workspaceId: string,
  agentId: string
): ChatKey {
  return makeChatKey(workspaceId, agentId)
}
