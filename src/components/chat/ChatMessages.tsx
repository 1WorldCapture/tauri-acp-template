import { useEffect, useRef, useMemo } from 'react'
import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'
import { UserMessage } from './UserMessage'
import { AIMessage } from './AIMessage'
import { ToolCallCard } from './ToolCallCard'
import type { ChatMessage, ToolCallEntry } from '@/store/chat-store'

interface ChatMessagesProps {
  messages: ChatMessage[]
  toolCalls?: ToolCallEntry[]
  sending?: boolean
  className?: string
}

/** Unified timeline item for sorting messages and tool calls together */
type TimelineItem =
  | {
      type: 'message'
      id: string
      createdAtMs: number
      seq?: number
      message: ChatMessage
    }
  | {
      type: 'toolCall'
      id: string
      createdAtMs: number
      seq?: number
      toolCall: ToolCallEntry
    }

export function ChatMessages({
  messages,
  toolCalls = [],
  sending = false,
  className,
}: ChatMessagesProps) {
  const bottomRef = useRef<HTMLDivElement>(null)

  // Create unified timeline of messages and tool calls, sorted by timestamp/seq
  const timeline = useMemo(() => {
    const items: TimelineItem[] = [
      ...messages.map(m => ({
        type: 'message' as const,
        id: m.id,
        createdAtMs: m.createdAtMs,
        seq: m.seq,
        message: m,
      })),
      ...toolCalls.map(t => ({
        type: 'toolCall' as const,
        id: t.toolCallId,
        createdAtMs: t.createdAtMs,
        seq: t.seq,
        toolCall: t,
      })),
    ]

    // Sort by createdAtMs, then seq, then type (messages before toolCalls as tiebreaker)
    return items.sort((a, b) => {
      if (a.createdAtMs !== b.createdAtMs) return a.createdAtMs - b.createdAtMs
      const aSeq = typeof a.seq === 'number' ? a.seq : -1
      const bSeq = typeof b.seq === 'number' ? b.seq : -1
      if (aSeq !== bSeq) return aSeq - bSeq
      // Tiebreaker: messages before toolCalls, then by id
      if (a.type !== b.type) return a.type === 'message' ? -1 : 1
      return a.id.localeCompare(b.id)
    })
  }, [messages, toolCalls])

  // Check if there's a streaming assistant message
  const hasStreamingAssistant = messages.some(
    m => m.role === 'assistant' && m.streaming
  )

  // Scroll to bottom when timeline changes
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
  }, [timeline, sending])

  if (timeline.length === 0 && !sending) {
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
        {timeline.map(item => {
          if (item.type === 'toolCall') {
            return (
              <ToolCallCard
                key={`toolcall-${item.id}`}
                toolCall={item.toolCall}
              />
            )
          }

          const message = item.message

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
