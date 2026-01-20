import { createContext } from 'react'
import type { ColorTheme } from '@/lib/theme-config'

export type Theme = 'dark' | 'light' | 'system'
export type { ColorTheme } from '@/lib/theme-config'

export interface ThemeProviderState {
  theme: Theme
  colorTheme: ColorTheme
  setTheme: (theme: Theme) => void
  setColorTheme: (colorTheme: ColorTheme) => void
}

const initialState: ThemeProviderState = {
  theme: 'system',
  colorTheme: 'default',
  setTheme: () => null,
  setColorTheme: () => null,
}

export const ThemeProviderContext =
  createContext<ThemeProviderState>(initialState)
