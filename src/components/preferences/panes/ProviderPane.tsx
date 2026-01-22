/**
 * Provider pane for the Preferences dialog.
 *
 * Displays a list of AI providers with their installation status
 * and allows users to install/manage providers.
 */

import { useState, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { ProviderCard } from './ProviderCard'
import { KNOWN_PROVIDERS, type ProviderConfig } from '@/config/providers'
import {
  usePluginStatus,
  useInstallPlugin,
  usePermissionRespond,
} from '@/services/plugins'
import {
  usePluginEvents,
  type AcpPermissionRequestedEvent,
  type AcpPluginStatusChangedEvent,
} from '@/hooks/usePluginEvents'
import { logger } from '@/lib/logger'

interface PendingPermission {
  operationId: string
  pluginId: string
  providerName: string
}

export function ProviderPane() {
  const { t } = useTranslation()
  const [installingPluginId, setInstallingPluginId] = useState<string | null>(
    null
  )
  const [pendingPermission, setPendingPermission] =
    useState<PendingPermission | null>(null)
  // Track if permission was already responded to (to prevent double-response)
  const permissionRespondedRef = useRef(false)

  const installPlugin = useInstallPlugin()
  const permissionRespond = usePermissionRespond()

  // Handle permission request events
  const handlePermissionRequested = useCallback(
    (event: AcpPermissionRequestedEvent) => {
      if (event.source.type === 'installPlugin') {
        const source = event.source as { type: 'installPlugin'; pluginId: string }
        const provider = KNOWN_PROVIDERS.find(
          p => p.pluginId === source.pluginId
        )
        // Reset the responded flag for new permission request
        permissionRespondedRef.current = false
        setPendingPermission({
          operationId: event.operationId,
          pluginId: source.pluginId,
          providerName: provider?.name ?? source.pluginId,
        })
      }
    },
    []
  )

  // Handle plugin status change events
  const handlePluginStatusChanged = useCallback(
    (event: AcpPluginStatusChangedEvent) => {
      // Only handle status changes for the plugin we're installing
      if (event.status.pluginId === installingPluginId) {
        setInstallingPluginId(null)

        if (event.error) {
          toast.error(t('toast.error.installFailed'), {
            description: event.error,
          })
        } else if (event.status.installed) {
          const provider = KNOWN_PROVIDERS.find(
            p => p.pluginId === event.status.pluginId
          )
          toast.success(
            t('toast.success.installComplete', {
              providerName: provider?.name ?? event.status.pluginId,
            })
          )
        }
      }
    },
    [installingPluginId, t]
  )

  // Set up event listeners
  usePluginEvents({
    onPermissionRequested: handlePermissionRequested,
    onPluginStatusChanged: handlePluginStatusChanged,
  })

  // Handle install button click
  const handleInstall = async (pluginId: string) => {
    setInstallingPluginId(pluginId)
    try {
      await installPlugin.mutateAsync({ pluginId })
      // Permission dialog will be shown via event listener
    } catch (error) {
      logger.error('Failed to start installation', { error, pluginId })
      toast.error(t('toast.error.installFailed'))
      setInstallingPluginId(null)
    }
  }

  // Handle permission allow
  const handlePermissionAllow = async () => {
    if (!pendingPermission || permissionRespondedRef.current) return
    permissionRespondedRef.current = true

    try {
      await permissionRespond.mutateAsync({
        operationId: pendingPermission.operationId,
        decision: 'allowOnce',
      })
    } catch (error) {
      logger.error('Failed to respond to permission', { error })
      toast.error(t('toast.error.installFailed'))
      setInstallingPluginId(null)
    }
    setPendingPermission(null)
  }

  // Handle permission deny
  const handlePermissionDeny = async () => {
    if (!pendingPermission || permissionRespondedRef.current) return
    permissionRespondedRef.current = true

    try {
      await permissionRespond.mutateAsync({
        operationId: pendingPermission.operationId,
        decision: 'deny',
      })
    } catch (error) {
      logger.error('Failed to respond to permission', { error })
    }
    setInstallingPluginId(null)
    setPendingPermission(null)
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h2 className="text-lg font-semibold text-foreground">
          {t('preferences.providers.title')}
        </h2>
        <p className="text-sm text-muted-foreground">
          {t('preferences.providers.description')}
        </p>
      </div>

      {/* Provider list */}
      <div className="space-y-3">
        {KNOWN_PROVIDERS.map(provider => (
          <ProviderCardWithStatus
            key={provider.pluginId}
            config={provider}
            isInstalling={installingPluginId === provider.pluginId}
            onInstall={() => handleInstall(provider.pluginId)}
          />
        ))}
      </div>

      {/* Permission Dialog */}
      <AlertDialog
        open={!!pendingPermission}
        onOpenChange={open => {
          if (!open) {
            handlePermissionDeny()
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('preferences.providers.permission.title')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('preferences.providers.permission.description', {
                providerName: pendingPermission?.providerName,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={handlePermissionDeny}>
              {t('preferences.providers.permission.deny')}
            </AlertDialogCancel>
            <AlertDialogAction onClick={handlePermissionAllow}>
              {t('preferences.providers.permission.allow')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

/**
 * Helper component that wraps ProviderCard with status query.
 * Separate component to allow per-provider query hook usage.
 */
function ProviderCardWithStatus({
  config,
  isInstalling,
  onInstall,
}: {
  config: ProviderConfig
  isInstalling: boolean
  onInstall: () => void
}) {
  const { data: status, isLoading } = usePluginStatus(config.pluginId)

  return (
    <ProviderCard
      config={config}
      status={status}
      isLoading={isLoading}
      isInstalling={isInstalling}
      onInstall={onInstall}
    />
  )
}
