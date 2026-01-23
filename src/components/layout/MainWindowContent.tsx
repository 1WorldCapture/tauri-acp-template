import { useEffect, useRef } from 'react'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { useUIStore } from '@/store/ui-store'
import {
  useChatStore,
  selectConversation,
  makeChatKeyFromIds,
} from '@/store/chat-store'
import { ChatArea } from '@/components/chat'
import { useProjectsList, getProjectName } from '@/services/projects'
import { useAgentsList, getAgentDisplayName } from '@/services/agents'
import { useChatSendPrompt, formatChatApiError } from '@/services/chat'

interface MainWindowContentProps {
  children?: React.ReactNode
  className?: string
}

export function MainWindowContent({
  children,
  className,
}: MainWindowContentProps) {
  const selectedAgentId = useUIStore(state => state.selectedAgentId)
  const selectedProjectId = useUIStore(state => state.selectedProjectId)

  // Track previous selection to detect changes
  const prevSelectionRef = useRef<{ projectId: string | null; agentId: string | null }>({
    projectId: null,
    agentId: null,
  })

  // Get project data
  const { data: projects } = useProjectsList()
  const selectedProject = projects?.find(
    p => p.workspaceId === selectedProjectId
  )
  const projectName = selectedProject
    ? getProjectName(selectedProject.rootDir)
    : undefined

  // Get agent data
  const { data: agents } = useAgentsList(selectedProjectId)
  const selectedAgent = agents?.find(a => a.agentId === selectedAgentId)
  const agentName = selectedAgent
    ? getAgentDisplayName(selectedAgent)
    : undefined

  // Get conversation from store
  const conversation = useChatStore(
    selectConversation(selectedProjectId, selectedAgentId)
  )

  // Chat store actions
  const ensureConversation = useChatStore(state => state.ensureConversation)
  const resetConversation = useChatStore(state => state.resetConversation)
  const addUserMessage = useChatStore(state => state.addUserMessage)
  const beginAssistantMessage = useChatStore(state => state.beginAssistantMessage)
  const setSessionId = useChatStore(state => state.setSessionId)
  const setSending = useChatStore(state => state.setSending)
  const setAssistantError = useChatStore(state => state.setAssistantError)

  // Send prompt mutation
  const sendPrompt = useChatSendPrompt()

  // Reset conversation when selection changes
  useEffect(() => {
    const prev = prevSelectionRef.current
    const selectionChanged =
      prev.projectId !== selectedProjectId || prev.agentId !== selectedAgentId

    if (selectionChanged && selectedProjectId && selectedAgentId) {
      // Ensure conversation exists and reset it
      const key = ensureConversation(selectedProjectId, selectedAgentId)
      resetConversation(key)
    }

    // Update ref
    prevSelectionRef.current = {
      projectId: selectedProjectId,
      agentId: selectedAgentId,
    }
  }, [selectedProjectId, selectedAgentId, ensureConversation, resetConversation])

  // Handle sending a message
  const handleSendMessage = async (prompt: string) => {
    if (!selectedProjectId || !selectedAgentId) return

    const key = makeChatKeyFromIds(selectedProjectId, selectedAgentId)

    // Add user message
    addUserMessage(key, prompt)

    // Begin assistant message (empty, streaming)
    beginAssistantMessage(key)

    // Mark as sending
    setSending(key, true)

    try {
      const ack = await sendPrompt.mutateAsync({
        workspaceId: selectedProjectId,
        agentId: selectedAgentId,
        prompt,
      })

      // Store session ID
      setSessionId(key, ack.sessionId)
    } catch (error) {
      // Mark error on the assistant message
      setAssistantError(key, formatChatApiError(error))
      toast.error('Failed to send message', {
        description: formatChatApiError(error),
      })
    }
  }

  // Show chat area only when both project and agent are selected
  if (selectedProjectId && selectedAgentId) {
    // Determine if input should be disabled
    const inputDisabled =
      conversation?.sending ||
      conversation?.agentStatus?.type === 'starting'

    return (
      <div className={cn('flex h-full flex-col', className)}>
        <ChatArea
          projectName={projectName}
          agentName={agentName}
          agentStatus={conversation?.agentStatus ?? undefined}
          messages={conversation?.messages ?? []}
          inputDisabled={inputDisabled}
          onSendMessage={handleSendMessage}
        />
      </div>
    )
  }

  // Default: show placeholder or children
  return (
    <div className={cn('flex h-full flex-col bg-background', className)}>
      {children || (
        <div className="flex flex-1 flex-col items-center justify-center">
          <h1 className="text-4xl font-bold text-foreground">
            {selectedProjectId
              ? 'Select an agent to start chatting'
              : 'Select a project to get started'}
          </h1>
        </div>
      )}
    </div>
  )
}
