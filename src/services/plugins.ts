/**
 * Plugin service layer for AI provider management.
 *
 * Provides TanStack Query hooks for:
 * - Getting plugin installation status
 * - Installing plugins (with permission flow)
 * - Responding to permission requests
 */

import { useQuery, useMutation } from '@tanstack/react-query'
import { logger } from '@/lib/logger'
import {
  commands,
  type PluginStatus,
  type PermissionDecision,
} from '@/lib/tauri-bindings'

// Query keys for plugins
export const pluginsQueryKeys = {
  all: ['plugins'] as const,
  status: (pluginId: string) =>
    [...pluginsQueryKeys.all, 'status', pluginId] as const,
}

/**
 * Hook to get plugin installation status.
 *
 * @param pluginId - Plugin identifier (e.g., "claude-code", "codex")
 * @returns TanStack Query result with PluginStatus
 */
export function usePluginStatus(pluginId: string) {
  return useQuery({
    queryKey: pluginsQueryKeys.status(pluginId),
    queryFn: async (): Promise<PluginStatus> => {
      logger.debug('Getting plugin status', { pluginId })
      const result = await commands.pluginGetStatus(pluginId, false)

      if (result.status === 'error') {
        logger.error('Failed to get plugin status', {
          pluginId,
          error: result.error,
        })
        throw result.error
      }

      logger.debug('Plugin status retrieved', {
        pluginId,
        status: result.data,
      })
      return result.data
    },
    staleTime: 1000 * 60 * 5, // 5 minutes
    gcTime: 1000 * 60 * 10, // 10 minutes
  })
}

/**
 * Hook to start plugin installation.
 *
 * This initiates an async installation process:
 * 1. Returns immediately with an operation ID
 * 2. Backend emits `acp/permission_requested` event
 * 3. On approval, backend installs and emits `acp/plugin_status_changed`
 *
 * @returns Mutation that returns { operationId: string }
 */
export function useInstallPlugin() {
  return useMutation({
    mutationFn: async ({
      pluginId,
      version,
    }: {
      pluginId: string
      version?: string
    }) => {
      logger.info('Starting plugin installation', { pluginId, version })
      const result = await commands.pluginInstall(pluginId, version ?? null)

      if (result.status === 'error') {
        logger.error('Failed to start plugin installation', {
          pluginId,
          error: result.error,
        })
        throw result.error
      }

      logger.info('Plugin installation started', {
        pluginId,
        operationId: result.data.operationId,
      })
      return result.data
    },
  })
}

/**
 * Hook to respond to a permission request.
 *
 * Called when user approves or denies a permission request
 * (e.g., plugin installation approval).
 *
 * @returns Mutation for permission response
 */
export function usePermissionRespond() {
  return useMutation({
    mutationFn: async ({
      operationId,
      decision,
    }: {
      operationId: string
      decision: PermissionDecision
    }) => {
      logger.info('Responding to permission request', { operationId, decision })
      const result = await commands.permissionRespond(operationId, decision)

      if (result.status === 'error') {
        logger.error('Failed to respond to permission', {
          operationId,
          error: result.error,
        })
        throw result.error
      }

      logger.info('Permission response sent', { operationId, decision })
    },
  })
}
