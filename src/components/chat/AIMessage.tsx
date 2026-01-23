import { cn } from '@/lib/utils'
import { AlertCircle, Loader2 } from 'lucide-react'

interface AIMessageProps {
  children: React.ReactNode
  streaming?: boolean
  error?: string
  className?: string
}

export function AIMessage({
  children,
  streaming,
  error,
  className,
}: AIMessageProps) {
  return (
    <div
      className={cn(
        'max-w-[615px] rounded-lg bg-card px-5 py-4',
        'border-l-4',
        error ? 'border-l-destructive' : 'border-l-primary',
        'text-[13px] leading-[1.6] text-card-foreground',
        className
      )}
    >
      {error ? (
        <div className="flex items-start gap-2 text-destructive">
          <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{error}</span>
        </div>
      ) : (
        <>
          {children}
          {streaming && (
            <span className="ml-1 inline-flex items-center">
              <Loader2 className="h-3 w-3 animate-spin text-muted-foreground" />
            </span>
          )}
        </>
      )}
    </div>
  )
}

interface AIMessageTextProps {
  children: React.ReactNode
  className?: string
}

export function AIMessageText({ children, className }: AIMessageTextProps) {
  return (
    <p className={cn('text-[13px] leading-[1.6]', className)}>{children}</p>
  )
}

interface AIMessageBulletsProps {
  items: string[]
  className?: string
}

export function AIMessageBullets({ items, className }: AIMessageBulletsProps) {
  return (
    <div className={cn('flex flex-col gap-2', className)}>
      {items.map((item, index) => (
        <p key={index} className="text-[13px] leading-[1.5]">
          {item}
        </p>
      ))}
    </div>
  )
}
