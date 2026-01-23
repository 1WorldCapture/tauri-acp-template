import { cn } from '@/lib/utils'
import { ChatHeader } from './ChatHeader'
import { ChatMessages } from './ChatMessages'
import { ChatInput } from './ChatInput'
import type { ChatMessage, AgentStatusLike } from '@/store/chat-store'

interface ChatAreaProps {
  projectName?: string
  agentName?: string
  agentStatus?: AgentStatusLike
  messages?: ChatMessage[]
  sending?: boolean
  inputDisabled?: boolean
  onSendMessage?: (message: string) => void
  className?: string
}

export function ChatArea({
  projectName,
  agentName,
  agentStatus,
  messages = [],
  sending = false,
  inputDisabled = false,
  onSendMessage,
  className,
}: ChatAreaProps) {
  return (
    <div
      className={cn('flex h-full min-h-0 flex-col bg-background', className)}
    >
      <ChatHeader
        projectName={projectName}
        agentName={agentName}
        agentStatus={agentStatus}
      />
      <ChatMessages messages={messages} sending={sending} />
      <ChatInput onSend={onSendMessage} disabled={inputDisabled} />
    </div>
  )
}
