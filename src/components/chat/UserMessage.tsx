import { cn } from '@/lib/utils'

interface UserMessageProps {
  children: React.ReactNode
  className?: string
}

export function UserMessage({ children, className }: UserMessageProps) {
  return (
    <div className="flex w-full justify-end">
      <div
        className={cn(
          'max-w-[600px] rounded-xl bg-secondary px-4 py-3',
          'text-[13px] leading-[1.6] text-secondary-foreground',
          className
        )}
      >
        {children}
      </div>
    </div>
  )
}
