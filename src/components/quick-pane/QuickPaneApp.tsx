import { useState, useEffect, useRef } from 'react'
import { emit, listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { commands } from '@/lib/tauri-bindings'
import { logger } from '@/lib/logger'
import {
  ALL_COLOR_THEME_CLASSES,
  COLOR_THEME_CLASSES,
  COLOR_THEME_STORAGE_KEY,
  THEME_STORAGE_KEY,
  isColorTheme,
} from '@/lib/theme-config'

/** Dismiss the quick pane window, logging any errors */
async function dismissQuickPane() {
  const result = await commands.dismissQuickPane()
  if (result.status === 'error') {
    logger.error('Failed to dismiss quick pane', { error: result.error })
  }
}

/**
 * QuickPaneApp - A minimal floating window for quick text entry.
 *
 * This component demonstrates the quick pane pattern:
 * - Single text input with submit on Enter
 * - Emits 'quick-pane-submit' event with the entered text
 * - Theme synced with main window via localStorage
 * - Hides window on submit or Escape
 */
// Apply theme from localStorage to document
function applyTheme() {
  const themeMode = localStorage.getItem(THEME_STORAGE_KEY) || 'system'
  const storedColorTheme = localStorage.getItem(COLOR_THEME_STORAGE_KEY)
  const colorTheme = isColorTheme(storedColorTheme)
    ? storedColorTheme
    : 'default'
  const root = document.documentElement

  root.classList.remove('light', 'dark', ...ALL_COLOR_THEME_CLASSES)

  const colorClass = COLOR_THEME_CLASSES[colorTheme]
  if (colorClass) {
    root.classList.add(colorClass)
  }

  if (themeMode === 'system') {
    const systemTheme = window.matchMedia('(prefers-color-scheme: dark)')
      .matches
      ? 'dark'
      : 'light'
    root.classList.add(systemTheme)
  } else {
    root.classList.add(themeMode)
  }
}

export default function QuickPaneApp() {
  const [text, setText] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  // Apply theme on mount and listen for theme changes from main window
  useEffect(() => {
    applyTheme()

    const unlisten = listen('theme-changed', () => {
      applyTheme()
    })

    return () => {
      unlisten.then(fn => fn())
    }
  }, [])

  // Focus input when window becomes visible, hide on blur
  useEffect(() => {
    const currentWindow = getCurrentWindow()
    const unlisten = currentWindow.onFocusChanged(
      async ({ payload: focused }) => {
        if (focused) {
          // Re-apply theme in case it changed while hidden
          applyTheme()
          inputRef.current?.focus()
        } else {
          // Hide window when it loses focus (dismiss on blur)
          // Use dismiss command for consistent behavior (no animation)
          await dismissQuickPane()
        }
      }
    )

    return () => {
      unlisten.then(fn => fn())
    }
  }, [])

  // Handle Escape key to dismiss
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault() // Prevent system "boop" sound
        await dismissQuickPane()
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()

    if (text.trim()) {
      // Emit the event for main window to handle
      await emit('quick-pane-submit', { text: text.trim() })
      setText('')
    }

    // Use dismiss command to avoid space switching on macOS
    await dismissQuickPane()
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex h-screen w-screen items-center rounded-xl border border-border bg-background px-5 shadow-lg"
    >
      <input
        ref={inputRef}
        type="text"
        value={text}
        onChange={e => setText(e.target.value)}
        placeholder="Enter text..."
        className="w-full bg-transparent text-lg text-foreground placeholder:text-muted-foreground outline-none"
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
      />
    </form>
  )
}
