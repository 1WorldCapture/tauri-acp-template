import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { logger } from '@/lib/logger'
import { commands, type AgentSummary } from '@/lib/tauri-bindings'

/**
 * Format ApiError for user-facing messages.
 *
 * Handles different ApiError variants from the Rust backend.
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
  if ('workspaceId' in e && typeof e.workspaceId === 'string') {
    return `Workspace not found: ${e.workspaceId}`
  }
  if ('agentId' in e && typeof e.agentId === 'string') {
    return `Agent not found: ${e.agentId}`
  }

  return 'An error occurred'
}

// Query keys for agents
export const agentsQueryKeys = {
  all: ['agents'] as const,
  list: (workspaceId: string) =>
    [...agentsQueryKeys.all, 'list', workspaceId] as const,
}

/**
 * Hook to fetch the list of agents for a workspace.
 */
export function useAgentsList(workspaceId: string | null) {
  return useQuery({
    queryKey: agentsQueryKeys.list(workspaceId ?? ''),
    queryFn: async (): Promise<AgentSummary[]> => {
      if (!workspaceId) {
        return []
      }

      logger.debug('Loading agents list from backend', { workspaceId })
      const result = await commands.agentList(workspaceId)

      if (result.status === 'error') {
        logger.error('Failed to load agents list', { error: result.error })
        throw result.error
      }

      logger.debug('Agents list loaded', {
        workspaceId,
        count: result.data.length,
      })
      return result.data
    },
    enabled: !!workspaceId,
    staleTime: 1000 * 30, // 30 seconds
    gcTime: 1000 * 60 * 5, // 5 minutes
  })
}

interface CreateAgentParams {
  workspaceId: string
  pluginId: string
  displayName?: string
}

/**
 * Hook to create a new agent in a workspace.
 */
export function useCreateAgent() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: async ({
      workspaceId,
      pluginId,
      displayName,
    }: CreateAgentParams): Promise<AgentSummary> => {
      logger.info('Creating agent', { workspaceId, pluginId, displayName })
      const result = await commands.agentCreate(
        workspaceId,
        pluginId,
        displayName ?? null
      )

      if (result.status === 'error') {
        logger.error('Failed to create agent', {
          error: result.error,
          workspaceId,
          pluginId,
        })
        throw result.error
      }

      logger.info('Agent created successfully', {
        agentId: result.data.agentId,
        workspaceId,
      })
      return result.data
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({
        queryKey: agentsQueryKeys.list(variables.workspaceId),
      })
      toast.success('Agent created')
    },
    onError: error => {
      toast.error('Failed to create agent', {
        description: formatApiError(error),
      })
    },
  })
}

/**
 * Get display name for an agent.
 * Falls back to a formatted plugin ID if no display name is set.
 */
export function getAgentDisplayName(agent: AgentSummary): string {
  if (agent.displayName) {
    return agent.displayName
  }

  // Format plugin ID as display name (e.g., "claude-code" -> "Claude Code")
  return agent.pluginId
    .split('-')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
}
