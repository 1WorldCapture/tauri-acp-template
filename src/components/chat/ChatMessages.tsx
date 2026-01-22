import { cn } from '@/lib/utils'
import { ScrollArea } from '@/components/ui/scroll-area'

interface ChatMessagesProps {
  children: React.ReactNode
  className?: string
}

export function ChatMessages({ children, className }: ChatMessagesProps) {
  return (
    <ScrollArea className="flex-1 bg-background">
      <div className={cn('flex flex-col gap-4 p-4', className)}>{children}</div>
    </ScrollArea>
  )
}
