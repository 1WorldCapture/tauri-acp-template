import { render, screen } from '@/test/test-utils'
import { describe, it, expect, vi } from 'vitest'
import App from './App'

// Tauri bindings are mocked globally in src/test/setup.ts

vi.mock('./i18n/language-init', () => ({
  initializeLanguage: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('./lib/menu', () => ({
  buildAppMenu: vi.fn().mockResolvedValue(undefined),
  setupMenuLanguageListener: vi.fn(),
}))

vi.mock('./lib/recovery', () => ({
  cleanupOldFiles: vi.fn().mockResolvedValue(undefined),
}))

describe('App', () => {
  it('renders main window layout', () => {
    render(<App />)
    expect(
      screen.getByRole('heading', { name: /select a project to get started/i })
    ).toBeInTheDocument()
  })

  it('renders title bar with traffic light buttons', () => {
    render(<App />)
    // Find specifically the window control buttons in the title bar
    const titleBarButtons = screen
      .getAllByRole('button')
      .filter(
        button =>
          button.getAttribute('aria-label')?.includes('window') ||
          button.className.includes('window-control')
      )
    // Should have at least the window control buttons
    expect(titleBarButtons.length).toBeGreaterThan(0)
  })
})
