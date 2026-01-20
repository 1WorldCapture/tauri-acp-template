import { useTranslation } from 'react-i18next'
import { locale } from '@tauri-apps/plugin-os'
import { toast } from 'sonner'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { useTheme } from '@/hooks/use-theme'
import { COLOR_THEMES } from '@/lib/theme-config'
import { SettingsField, SettingsSection } from '../shared/SettingsComponents'
import { usePreferences, useSavePreferences } from '@/services/preferences'
import { availableLanguages } from '@/i18n'
import { logger } from '@/lib/logger'

// Language display names (native names)
const languageNames: Record<string, string> = {
  en: 'English',
  fr: 'Français',
  ar: 'العربية',
}

export function AppearancePane() {
  const { t, i18n } = useTranslation()
  const { theme, colorTheme, setTheme, setColorTheme } = useTheme()
  const { data: preferences } = usePreferences()
  const savePreferences = useSavePreferences()

  const colorThemeLabels: Record<(typeof COLOR_THEMES)[number], string> = {
    default: t('preferences.appearance.colorTheme.default'),
    claude: t('preferences.appearance.colorTheme.claude'),
    perplexity: t('preferences.appearance.colorTheme.perplexity'),
    'cosmic-night': t('preferences.appearance.colorTheme.cosmic-night'),
    'modern-minimal': t('preferences.appearance.colorTheme.modern-minimal'),
    'ocean-breeze': t('preferences.appearance.colorTheme.ocean-breeze'),
  }

  const handleThemeChange = (value: 'light' | 'dark' | 'system') => {
    // Update the theme provider immediately for instant UI feedback
    setTheme(value)

    // Persist the theme preference to disk, preserving other preferences
    if (preferences) {
      savePreferences.mutate({ ...preferences, theme: value })
    }
  }

  const handleColorThemeChange = (value: (typeof COLOR_THEMES)[number]) => {
    setColorTheme(value)

    if (preferences) {
      savePreferences.mutate({ ...preferences, color_theme: value })
    }
  }

  const handleLanguageChange = async (value: string) => {
    const language = value === 'system' ? null : value

    try {
      // Change the language immediately for instant UI feedback
      if (language) {
        await i18n.changeLanguage(language)
      } else {
        // System language selected - detect and apply system locale
        const systemLocale = await locale()
        const langCode = systemLocale?.split('-')[0]?.toLowerCase() ?? 'en'
        const targetLang = availableLanguages.includes(langCode)
          ? langCode
          : 'en'
        await i18n.changeLanguage(targetLang)
      }
    } catch (error) {
      logger.error('Failed to change language', { error })
      toast.error(t('toast.error.generic'))
      return
    }

    // Persist the language preference to disk
    if (preferences) {
      savePreferences.mutate({ ...preferences, language })
    }
  }

  // Determine the current language value for the select
  const currentLanguageValue = preferences?.language ?? 'system'

  return (
    <div className="space-y-6">
      <SettingsSection title={t('preferences.appearance.language')}>
        <SettingsField
          label={t('preferences.appearance.language')}
          description={t('preferences.appearance.languageDescription')}
        >
          <Select
            value={currentLanguageValue}
            onValueChange={handleLanguageChange}
            disabled={savePreferences.isPending}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="system">
                {t('preferences.appearance.language.system')}
              </SelectItem>
              {availableLanguages.map(lang => (
                <SelectItem key={lang} value={lang}>
                  {languageNames[lang] ?? lang}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SettingsField>
      </SettingsSection>

      <SettingsSection title={t('preferences.appearance.theme')}>
        <SettingsField
          label={t('preferences.appearance.mode')}
          description={t('preferences.appearance.modeDescription')}
        >
          <Select
            value={theme}
            onValueChange={handleThemeChange}
            disabled={savePreferences.isPending}
          >
            <SelectTrigger>
              <SelectValue
                placeholder={t('preferences.appearance.selectMode')}
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="light">
                {t('preferences.appearance.theme.light')}
              </SelectItem>
              <SelectItem value="dark">
                {t('preferences.appearance.theme.dark')}
              </SelectItem>
              <SelectItem value="system">
                {t('preferences.appearance.theme.system')}
              </SelectItem>
            </SelectContent>
          </Select>
        </SettingsField>

        <SettingsField
          label={t('preferences.appearance.colorTheme')}
          description={t('preferences.appearance.colorThemeDescription')}
        >
          <Select
            value={colorTheme}
            onValueChange={handleColorThemeChange}
            disabled={savePreferences.isPending}
          >
            <SelectTrigger>
              <SelectValue
                placeholder={t('preferences.appearance.selectColorTheme')}
              />
            </SelectTrigger>
            <SelectContent>
              {COLOR_THEMES.map(themeKey => (
                <SelectItem key={themeKey} value={themeKey}>
                  {colorThemeLabels[themeKey]}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SettingsField>
      </SettingsSection>
    </div>
  )
}
