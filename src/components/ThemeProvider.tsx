import { useEffect, useLayoutEffect, useState, useRef } from 'react'
import { emit } from '@tauri-apps/api/event'
import {
  ThemeProviderContext,
  type ColorTheme,
  type Theme,
} from '@/lib/theme-context'
import {
  ALL_COLOR_THEME_CLASSES,
  COLOR_THEME_CLASSES,
  COLOR_THEME_STORAGE_KEY,
  THEME_STORAGE_KEY,
  isColorTheme,
} from '@/lib/theme-config'
import { usePreferences } from '@/services/preferences'

interface ThemeProviderProps {
  children: React.ReactNode
  defaultTheme?: Theme
  defaultColorTheme?: ColorTheme
  storageKey?: string
  colorStorageKey?: string
}

export function ThemeProvider({
  children,
  defaultTheme = 'system',
  defaultColorTheme = 'default',
  storageKey = THEME_STORAGE_KEY,
  colorStorageKey = COLOR_THEME_STORAGE_KEY,
  ...props
}: ThemeProviderProps) {
  const [theme, setThemeState] = useState<Theme>(() => {
    const storedTheme = localStorage.getItem(storageKey)
    if (
      storedTheme === 'light' ||
      storedTheme === 'dark' ||
      storedTheme === 'system'
    ) {
      return storedTheme
    }
    return defaultTheme
  })

  const [colorTheme, setColorThemeState] = useState<ColorTheme>(() => {
    const storedColorTheme = localStorage.getItem(colorStorageKey)
    if (isColorTheme(storedColorTheme)) {
      return storedColorTheme
    }
    return defaultColorTheme
  })

  // Load theme from persistent preferences
  const { data: preferences } = usePreferences()
  const hasSyncedPreferences = useRef(false)

  // Sync theme with preferences when they load
  // This is a legitimate case of syncing with external async state (persistent preferences)
  // The ref ensures this only happens once when preferences first load
  useLayoutEffect(() => {
    if (!preferences || hasSyncedPreferences.current) {
      return
    }

    const nextTheme =
      preferences.theme === 'light' ||
      preferences.theme === 'dark' ||
      preferences.theme === 'system'
        ? preferences.theme
        : defaultTheme

    const nextColorTheme = isColorTheme(preferences.color_theme)
      ? preferences.color_theme
      : defaultColorTheme

    hasSyncedPreferences.current = true
    localStorage.setItem(storageKey, nextTheme)
    localStorage.setItem(colorStorageKey, nextColorTheme)

    queueMicrotask(() => {
      setThemeState(nextTheme)
      setColorThemeState(nextColorTheme)
    })
    emit('theme-changed', { theme: nextTheme, colorTheme: nextColorTheme })
  }, [
    colorStorageKey,
    defaultColorTheme,
    defaultTheme,
    preferences,
    storageKey,
  ])

  useEffect(() => {
    const root = window.document.documentElement
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')

    const applyTheme = (isDark: boolean) => {
      root.classList.remove('light', 'dark')
      root.classList.add(isDark ? 'dark' : 'light')
    }

    if (theme === 'system') {
      applyTheme(mediaQuery.matches)

      const handleChange = (e: MediaQueryListEvent) => applyTheme(e.matches)
      mediaQuery.addEventListener('change', handleChange)
      return () => mediaQuery.removeEventListener('change', handleChange)
    }

    applyTheme(theme === 'dark')
  }, [theme])

  useEffect(() => {
    const root = window.document.documentElement
    root.classList.remove(...ALL_COLOR_THEME_CLASSES)

    const colorClass = COLOR_THEME_CLASSES[colorTheme]
    if (colorClass) {
      root.classList.add(colorClass)
    }
  }, [colorTheme])

  const value = {
    theme,
    colorTheme,
    setTheme: (newTheme: Theme) => {
      localStorage.setItem(storageKey, newTheme)
      setThemeState(newTheme)
      // Notify other windows (e.g., quick pane) of theme change
      emit('theme-changed', { theme: newTheme, colorTheme })
    },
    setColorTheme: (newColorTheme: ColorTheme) => {
      localStorage.setItem(colorStorageKey, newColorTheme)
      setColorThemeState(newColorTheme)
      emit('theme-changed', { theme, colorTheme: newColorTheme })
    },
  }

  return (
    <ThemeProviderContext.Provider {...props} value={value}>
      {children}
    </ThemeProviderContext.Provider>
  )
}
