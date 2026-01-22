import { cn } from '@/lib/utils'

interface AIMessageProps {
  children: React.ReactNode
  className?: string
}

export function AIMessage({ children, className }: AIMessageProps) {
  return (
    <div
      className={cn(
        'max-w-[615px] rounded-lg bg-card px-5 py-4',
        'border-l-4 border-l-primary',
        'text-[13px] leading-[1.6] text-card-foreground',
        className
      )}
    >
      {children}
    </div>
  )
}

interface AIMessageTextProps {
  children: React.ReactNode
  className?: string
}

export function AIMessageText({ children, className }: AIMessageTextProps) {
  return <p className={cn('text-[13px] leading-[1.6]', className)}>{children}</p>
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
