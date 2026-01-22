/**
 * Provider configuration for AI agent adapters (ACP plugins).
 *
 * This defines the known providers that can be installed and managed
 * through the Settings > Providers page.
 */

/**
 * Configuration for a provider (AI agent adapter).
 */
export interface ProviderConfig {
  /** Plugin identifier used in backend commands */
  pluginId: string
  /** Display name */
  name: string
  /** i18n key for description */
  descriptionKey: string
  /** Letter to display in icon */
  iconLetter: string
  /** Tailwind background color class for icon */
  iconBgColor: string
}

/**
 * Known providers available for installation.
 *
 * These are displayed in the Settings > Providers page.
 */
export const KNOWN_PROVIDERS: ProviderConfig[] = [
  {
    pluginId: 'claude-code',
    name: 'Claude Code',
    descriptionKey: 'preferences.providers.claudeCode.description',
    iconLetter: 'C',
    iconBgColor: 'bg-orange-500',
  },
  {
    pluginId: 'codex',
    name: 'Codex CLI',
    descriptionKey: 'preferences.providers.codex.description',
    iconLetter: 'O',
    iconBgColor: 'bg-blue-500',
  },
]
