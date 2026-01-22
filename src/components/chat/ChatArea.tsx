import { cn } from '@/lib/utils'
import { ChatHeader } from './ChatHeader'
import { ChatMessages } from './ChatMessages'
import { ChatInput } from './ChatInput'

interface ChatAreaProps {
  projectName?: string
  agentName?: string
  children?: React.ReactNode
  onSendMessage?: (message: string) => void
  className?: string
}

export function ChatArea({
  projectName,
  agentName,
  children,
  onSendMessage,
  className,
}: ChatAreaProps) {
  return (
    <div className={cn('flex h-full flex-col bg-background', className)}>
      <ChatHeader projectName={projectName} agentName={agentName} />
      <ChatMessages>{children}</ChatMessages>
      <ChatInput onSend={onSendMessage} />
    </div>
  )
}
