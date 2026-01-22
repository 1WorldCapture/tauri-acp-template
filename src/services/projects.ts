import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { toast } from 'sonner'
import { logger } from '@/lib/logger'
import { commands, type WorkspaceSummary } from '@/lib/tauri-bindings'

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
  if ('path' in e && typeof e.path === 'string') {
    return `Path error: ${e.path}`
  }
  if ('workspaceId' in e && typeof e.workspaceId === 'string') {
    return `Workspace not found: ${e.workspaceId}`
  }

  return 'An error occurred'
}

// Query keys for projects
export const projectsQueryKeys = {
  all: ['projects'] as const,
  list: () => [...projectsQueryKeys.all, 'list'] as const,
}

/**
 * Hook to fetch the list of all projects (workspaces).
 */
export function useProjectsList() {
  return useQuery({
    queryKey: projectsQueryKeys.list(),
    queryFn: async (): Promise<WorkspaceSummary[]> => {
      logger.debug('Loading projects list from backend')
      const result = await commands.workspaceList()

      if (result.status === 'error') {
        logger.error('Failed to load projects list', { error: result.error })
        throw result.error
      }

      logger.debug('Projects list loaded', { count: result.data.length })
      return result.data
    },
    staleTime: 1000 * 30, // 30 seconds
    gcTime: 1000 * 60 * 5, // 5 minutes
  })
}

/**
 * Hook to create a new project (workspace).
 */
export function useCreateProject() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: async (rootDir: string): Promise<WorkspaceSummary> => {
      logger.info('Creating project', { rootDir })
      const result = await commands.workspaceCreate(rootDir)

      if (result.status === 'error') {
        logger.error('Failed to create project', {
          error: result.error,
          rootDir,
        })
        throw result.error
      }

      logger.info('Project created successfully', {
        workspaceId: result.data.workspaceId,
      })
      return result.data
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: projectsQueryKeys.list() })
      toast.success('Project created')
    },
    onError: error => {
      toast.error('Failed to create project', {
        description: formatApiError(error),
      })
    },
  })
}

/**
 * Hook to delete a project (workspace).
 */
export function useDeleteProject() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: async (workspaceId: string): Promise<void> => {
      logger.info('Deleting project', { workspaceId })
      const result = await commands.workspaceDelete(workspaceId)

      if (result.status === 'error') {
        logger.error('Failed to delete project', {
          error: result.error,
          workspaceId,
        })
        throw result.error
      }

      logger.info('Project deleted successfully', { workspaceId })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: projectsQueryKeys.list() })
      toast.success('Project deleted')
    },
    onError: error => {
      toast.error('Failed to delete project', {
        description: formatApiError(error),
      })
    },
  })
}

/**
 * Extract project name from root directory path.
 */
export function getProjectName(rootDir: string): string {
  const parts = rootDir.split(/[\\/]/).filter(Boolean)
  return parts.pop() ?? rootDir
}
