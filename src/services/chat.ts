import { useMutation } from '@tanstack/react-query'
import { logger } from '@/lib/logger'
import { commands, type SendPromptAck } from '@/lib/tauri-bindings'

/**
 * Format ApiError for user-facing messages.
 */
function formatApiError(error: unknown): string {
  if (!error || typeof error !== 'object') {
    return 'Unknown error'
  }

  const e = error as Record<string, unknown>

  // Handle different ApiError variants
  if ('message' in e && typeof e.message === 'string') {
    return e.message
  }
  if (e.type === 'pluginNotInstalled' && 'pluginId' in e) {
    return `Plugin "${e.pluginId}" is not installed. Please install it first.`
  }
  if (e.type === 'pluginMissingBinPath' && 'pluginId' in e) {
    return `Plugin "${e.pluginId}" is missing its binary. Try reinstalling.`
  }
  if (e.type === 'protocolError') {
    return 'Failed to communicate with the agent'
  }
  if ('workspaceId' in e && typeof e.workspaceId === 'string') {
    return `Workspace not found: ${e.workspaceId}`
  }
  if ('agentId' in e && typeof e.agentId === 'string') {
    return `Agent not found: ${e.agentId}`
  }

  return 'An error occurred'
}

interface SendPromptParams {
  workspaceId: string
  agentId: string
  prompt: string
}

/**
 * Hook to send a prompt to an agent.
 *
 * This triggers lazy startup if the agent isn't running yet.
 * Responses arrive via `acp/session_update` events (handled by useAgentChatEvents).
 *
 * @example
 * ```typescript
 * const sendPrompt = useChatSendPrompt()
 *
 * // In a handler
 * sendPrompt.mutate(
 *   { workspaceId, agentId, prompt },
 *   {
 *     onSuccess: (ack) => {
 *       console.log('Session ID:', ack.sessionId)
 *     },
 *     onError: (error) => {
 *       console.error('Send failed:', error)
 *     },
 *   }
 * )
 * ```
 */
export function useChatSendPrompt() {
  return useMutation({
    mutationFn: async ({
      workspaceId,
      agentId,
      prompt,
    }: SendPromptParams): Promise<SendPromptAck> => {
      logger.info('Sending prompt to agent', {
        workspaceId,
        agentId,
        promptLength: prompt.length,
      })

      const result = await commands.chatSendPrompt(workspaceId, agentId, prompt)

      if (result.status === 'error') {
        logger.error('Failed to send prompt', {
          error: result.error,
          workspaceId,
          agentId,
        })
        throw result.error
      }

      logger.info('Prompt sent successfully', {
        sessionId: result.data.sessionId,
        workspaceId,
        agentId,
      })

      return result.data
    },
    onError: error => {
      // Log formatted error for debugging
      logger.error('Chat send prompt mutation error', {
        error,
        formatted: formatApiError(error),
      })
    },
  })
}

export { formatApiError as formatChatApiError }
