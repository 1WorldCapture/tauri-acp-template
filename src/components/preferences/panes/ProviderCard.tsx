/**
 * Provider card component for displaying AI provider status and actions.
 *
 * Displays:
 * - Provider icon (colored background with letter)
 * - Name and description
 * - Installation status indicator
 * - Install/Installed action button
 */

import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/utils'
import type { ProviderConfig } from '@/config/providers'
import type { PluginStatus } from '@/lib/tauri-bindings'

interface ProviderCardProps {
  /** Provider configuration with display metadata */
  config: ProviderConfig
  /** Plugin status from backend (undefined while loading) */
  status: PluginStatus | undefined
  /** Whether status is being loaded */
  isLoading: boolean
  /** Whether installation is in progress */
  isInstalling: boolean
  /** Callback when user clicks Install */
  onInstall: () => void
}

export function ProviderCard({
  config,
  status,
  isLoading,
  isInstalling,
  onInstall,
}: ProviderCardProps) {
  const { t } = useTranslation()

  const isInstalled = status?.installed ?? false
  const version = status?.installedVersion

  return (
    <div className="flex items-center gap-4 rounded-xl border bg-card p-4">
      {/* Icon - 48x48 colored background with letter */}
      <div
        className={cn(
          'flex size-12 shrink-0 items-center justify-center rounded-lg text-xl font-bold text-white',
          config.iconBgColor
        )}
      >
        {config.iconLetter}
      </div>

      {/* Info section */}
      <div className="min-w-0 flex-1">
        <div className="font-semibold text-foreground">{config.name}</div>
        <div className="text-sm text-muted-foreground">
          {t(config.descriptionKey)}
        </div>

        {/* Status indicator */}
        <div className="mt-1 flex items-center gap-1.5 text-xs">
          <span
            className={cn(
              'size-1.5 rounded-full',
              isInstalled ? 'bg-green-500' : 'bg-muted-foreground/50'
            )}
          />
          <span
            className={cn(
              isInstalled ? 'text-green-600 dark:text-green-500' : 'text-muted-foreground'
            )}
          >
            {isInstalled
              ? version
                ? `${t('preferences.providers.status.installed')} · ${t('preferences.providers.status.version', { version })}`
                : t('preferences.providers.status.installed')
              : t('preferences.providers.status.notInstalled')}
          </span>
        </div>
      </div>

      {/* Action button */}
      <Button
        variant={isInstalled ? 'secondary' : 'default'}
        size="sm"
        disabled={isInstalled || isInstalling || isLoading}
        onClick={onInstall}
        className={cn(
          'shrink-0',
          !isInstalled && !isInstalling && 'bg-orange-500 hover:bg-orange-600'
        )}
      >
        {isInstalling && <Spinner className="mr-2 size-3" />}
        {isInstalling
          ? t('preferences.providers.action.installing')
          : isInstalled
            ? t('preferences.providers.action.installed')
            : t('preferences.providers.action.install')}
      </Button>
    </div>
  )
}
