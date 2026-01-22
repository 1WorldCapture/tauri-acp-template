/**
 * Plugin event listener hook for ACP permission and status events.
 *
 * Listens for:
 * - `acp/permission_requested` - When backend needs user approval
 * - `acp/plugin_status_changed` - When plugin installation completes/fails
 */

import { useEffect, useCallback, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useQueryClient } from '@tanstack/react-query'
import { pluginsQueryKeys } from '@/services/plugins'
import { logger } from '@/lib/logger'
import type { PluginStatus } from '@/lib/tauri-bindings'

/**
 * Permission source for plugin installation.
 * Matches Rust PermissionSource::InstallPlugin variant.
 */
interface PermissionSourceInstallPlugin {
  type: 'installPlugin'
  pluginId: string
  version: string | null
}

/**
 * Permission request event payload.
 * Matches Rust AcpPermissionRequestedEvent.
 */
export interface AcpPermissionRequestedEvent {
  operationId: string
  source: PermissionSourceInstallPlugin | { type: string }
  requestedAtMs: number
  origin: {
    workspaceId?: string
    agentId?: string
    sessionId?: string
    toolCallId?: string
  } | null
}

/**
 * Plugin status changed event payload.
 * Matches Rust AcpPluginStatusChangedEvent.
 */
export interface AcpPluginStatusChangedEvent {
  operationId: string
  status: PluginStatus
  error: string | null
}

interface UsePluginEventsOptions {
  /**
   * Called when a permission request is received for plugin installation.
   */
  onPermissionRequested?: (event: AcpPermissionRequestedEvent) => void
  /**
   * Called when plugin status changes (installation complete/failed).
   */
  onPluginStatusChanged?: (event: AcpPluginStatusChangedEvent) => void
}

/**
 * Hook to listen for plugin-related events from the backend.
 *
 * Automatically invalidates plugin status queries when status changes.
 *
 * @example
 * ```typescript
 * usePluginEvents({
 *   onPermissionRequested: (event) => {
 *     if (event.source.type === 'installPlugin') {
 *       setPendingPermission({
 *         operationId: event.operationId,
 *         pluginId: event.source.pluginId,
 *       })
 *     }
 *   },
 *   onPluginStatusChanged: (event) => {
 *     if (event.error) {
 *       toast.error('Installation failed')
 *     }
 *   },
 * })
 * ```
 */
export function usePluginEvents(options: UsePluginEventsOptions = {}) {
  const queryClient = useQueryClient()

  // Use refs to avoid stale closure issues with callbacks
  const onPermissionRequestedRef = useRef(options.onPermissionRequested)
  const onPluginStatusChangedRef = useRef(options.onPluginStatusChanged)

  // Update refs when callbacks change
  useEffect(() => {
    onPermissionRequestedRef.current = options.onPermissionRequested
    onPluginStatusChangedRef.current = options.onPluginStatusChanged
  }, [options.onPermissionRequested, options.onPluginStatusChanged])

  // Stable handlers that read from refs
  const handlePermissionRequested = useCallback(
    (event: AcpPermissionRequestedEvent) => {
      onPermissionRequestedRef.current?.(event)
    },
    []
  )

  const handlePluginStatusChanged = useCallback(
    (event: AcpPluginStatusChangedEvent) => {
      // Invalidate the plugin status query
      queryClient.invalidateQueries({
        queryKey: pluginsQueryKeys.status(event.status.pluginId),
      })
      onPluginStatusChangedRef.current?.(event)
    },
    [queryClient]
  )

  useEffect(() => {
    let isMounted = true
    const unlisteners: (() => void)[] = []

    // Listen for permission requests
    listen<AcpPermissionRequestedEvent>('acp/permission_requested', event => {
      logger.debug('Permission requested event received', {
        payload: event.payload,
      })

      // Only handle InstallPlugin source
      if (event.payload.source.type === 'installPlugin') {
        handlePermissionRequested(event.payload)
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
        logger.error('Failed to setup permission_requested listener', { error })
      })

    // Listen for plugin status changes
    listen<AcpPluginStatusChangedEvent>('acp/plugin_status_changed', event => {
      logger.debug('Plugin status changed event received', {
        payload: event.payload,
      })
      handlePluginStatusChanged(event.payload)
    })
      .then(unlisten => {
        if (!isMounted) {
          unlisten()
        } else {
          unlisteners.push(unlisten)
        }
      })
      .catch(error => {
        logger.error('Failed to setup plugin_status_changed listener', {
          error,
        })
      })

    return () => {
      isMounted = false
      unlisteners.forEach(unlisten => unlisten())
    }
  }, [handlePermissionRequested, handlePluginStatusChanged])
}
