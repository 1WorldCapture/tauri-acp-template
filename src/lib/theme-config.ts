export const COLOR_THEMES = ['default', 'claude', 'perplexity'] as const
export type ColorTheme = (typeof COLOR_THEMES)[number]

export const COLOR_THEME_CLASSES: Record<ColorTheme, string> = {
  default: '',
  claude: 'theme-claude',
  perplexity: 'theme-perplexity',
}

export const ALL_COLOR_THEME_CLASSES =
  Object.values(COLOR_THEME_CLASSES).filter(Boolean)

export const THEME_STORAGE_KEY = 'ui-theme'
export const COLOR_THEME_STORAGE_KEY = 'ui-color-theme'

export const isColorTheme = (
  value: string | null | undefined
): value is ColorTheme =>
  !!value && (COLOR_THEMES as readonly string[]).includes(value)
