import { create } from 'zustand'
import { devtools } from 'zustand/middleware'

interface UIState {
  leftSidebarVisible: boolean
  rightSidebarVisible: boolean
  commandPaletteOpen: boolean
  preferencesOpen: boolean
  lastQuickPaneEntry: string | null
  selectedProjectId: string | null
  projectPendingDeleteId: string | null
  selectedAgentId: string | null

  toggleLeftSidebar: () => void
  setLeftSidebarVisible: (visible: boolean) => void
  toggleRightSidebar: () => void
  setRightSidebarVisible: (visible: boolean) => void
  toggleCommandPalette: () => void
  setCommandPaletteOpen: (open: boolean) => void
  togglePreferences: () => void
  setPreferencesOpen: (open: boolean) => void
  setLastQuickPaneEntry: (text: string) => void
  setSelectedProjectId: (id: string | null) => void
  setProjectPendingDeleteId: (id: string | null) => void
  setSelectedAgentId: (id: string | null) => void
  /** Select an agent with its project - ensures both IDs are set together */
  selectAgent: (projectId: string, agentId: string) => void
  /** Clear agent selection */
  clearAgentSelection: () => void
}

export const useUIStore = create<UIState>()(
  devtools(
    set => ({
      leftSidebarVisible: true,
      rightSidebarVisible: true,
      commandPaletteOpen: false,
      preferencesOpen: false,
      lastQuickPaneEntry: null,
      selectedProjectId: null,
      projectPendingDeleteId: null,
      selectedAgentId: null,

      toggleLeftSidebar: () =>
        set(
          state => ({ leftSidebarVisible: !state.leftSidebarVisible }),
          undefined,
          'toggleLeftSidebar'
        ),

      setLeftSidebarVisible: visible =>
        set(
          { leftSidebarVisible: visible },
          undefined,
          'setLeftSidebarVisible'
        ),

      toggleRightSidebar: () =>
        set(
          state => ({ rightSidebarVisible: !state.rightSidebarVisible }),
          undefined,
          'toggleRightSidebar'
        ),

      setRightSidebarVisible: visible =>
        set(
          { rightSidebarVisible: visible },
          undefined,
          'setRightSidebarVisible'
        ),

      toggleCommandPalette: () =>
        set(
          state => ({ commandPaletteOpen: !state.commandPaletteOpen }),
          undefined,
          'toggleCommandPalette'
        ),

      setCommandPaletteOpen: open =>
        set({ commandPaletteOpen: open }, undefined, 'setCommandPaletteOpen'),

      togglePreferences: () =>
        set(
          state => ({ preferencesOpen: !state.preferencesOpen }),
          undefined,
          'togglePreferences'
        ),

      setPreferencesOpen: open =>
        set({ preferencesOpen: open }, undefined, 'setPreferencesOpen'),

      setLastQuickPaneEntry: text =>
        set({ lastQuickPaneEntry: text }, undefined, 'setLastQuickPaneEntry'),

      setSelectedProjectId: id =>
        set(
          { selectedProjectId: id, selectedAgentId: null },
          undefined,
          'setSelectedProjectId'
        ),

      setProjectPendingDeleteId: id =>
        set(
          { projectPendingDeleteId: id },
          undefined,
          'setProjectPendingDeleteId'
        ),

      setSelectedAgentId: id =>
        set({ selectedAgentId: id }, undefined, 'setSelectedAgentId'),

      selectAgent: (projectId, agentId) =>
        set(
          { selectedProjectId: projectId, selectedAgentId: agentId },
          undefined,
          'selectAgent'
        ),

      clearAgentSelection: () =>
        set({ selectedAgentId: null }, undefined, 'clearAgentSelection'),
    }),
    {
      name: 'ui-store',
    }
  )
)
