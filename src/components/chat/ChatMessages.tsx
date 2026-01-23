import { useEffect, useRef } from 'react'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { UserMessage } from './UserMessage'
import { AIMessage } from './AIMessage'
import type { ChatMessage } from '@/store/chat-store'

interface ChatMessagesProps {
  messages: ChatMessage[]
  sending?: boolean
  className?: string
}

export function ChatMessages({
  messages,
  sending = false,
  className,
}: ChatMessagesProps) {
  const bottomRef = useRef<HTMLDivElement>(null)

  // Sort messages by timestamp and sequence for deterministic ordering
  const orderedMessages = [...messages].sort((a, b) => {
    if (a.createdAtMs !== b.createdAtMs) return a.createdAtMs - b.createdAtMs
    const aSeq = typeof a.seq === 'number' ? a.seq : -1
    const bSeq = typeof b.seq === 'number' ? b.seq : -1
    return aSeq - bSeq
  })

  // Check if there's a streaming assistant message
  const hasStreamingAssistant = orderedMessages.some(
    m => m.role === 'assistant' && m.streaming
  )

  // Scroll to bottom when messages change
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
  }, [messages, sending])

  if (orderedMessages.length === 0 && !sending) {
    return (
      <div className="flex flex-1 items-center justify-center bg-background">
        <p className="text-muted-foreground">
          Start a conversation by sending a message
        </p>
      </div>
    )
  }

  return (
    <ScrollArea className="min-h-0 flex-1 bg-background">
      <div className={cn('flex flex-col gap-4 p-4', className)}>
        {orderedMessages.map(message => {
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

        {sending && !hasStreamingAssistant && (
          <AIMessage streaming>
            <span className="text-muted-foreground">Thinking...</span>
          </AIMessage>
        )}

        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  )
}
