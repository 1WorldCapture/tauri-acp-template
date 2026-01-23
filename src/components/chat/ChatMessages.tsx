import { useEffect, useRef } from 'react'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { UserMessage } from './UserMessage'
import { AIMessage } from './AIMessage'
import type { ChatMessage } from '@/store/chat-store'

interface ChatMessagesProps {
  messages: ChatMessage[]
  className?: string
}

export function ChatMessages({ messages, className }: ChatMessagesProps) {
  const bottomRef = useRef<HTMLDivElement>(null)

  // Scroll to bottom when messages change
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
  }, [messages])

  if (messages.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center bg-background">
        <p className="text-muted-foreground">
          Start a conversation by sending a message
        </p>
      </div>
    )
  }

  return (
    <ScrollArea className="flex-1 bg-background">
      <div className={cn('flex flex-col gap-4 p-4', className)}>
        {messages.map(message => {
          if (message.role === 'user') {
            return (
              <UserMessage key={message.id}>
                <p className="whitespace-pre-wrap">{message.text}</p>
              </UserMessage>
            )
          }

          if (message.role === 'assistant') {
            return (
              <AIMessage
                key={message.id}
                streaming={message.streaming}
                error={message.error}
              >
                {message.text ? (
                  <p className="whitespace-pre-wrap">{message.text}</p>
                ) : message.streaming ? (
                  <span className="text-muted-foreground">Thinking...</span>
                ) : null}
              </AIMessage>
            )
          }

          // System messages (if any)
          return (
            <div
              key={message.id}
              className="rounded-md bg-muted p-3 text-sm text-muted-foreground"
            >
              <p className="whitespace-pre-wrap">{message.text}</p>
            </div>
          )
        })}
        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  )
}
