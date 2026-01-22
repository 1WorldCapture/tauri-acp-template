import { useState } from 'react'
import { cn } from '@/lib/utils'
import { ChevronDown, Plus, ArrowUp } from 'lucide-react'

interface ChatInputProps {
  onSend?: (message: string) => void
  placeholder?: string
  modelName?: string
  modeName?: string
  disabled?: boolean
  className?: string
}

export function ChatInput({
  onSend,
  placeholder = 'Ask Codex to do something...',
  modelName = 'gpt-5.2',
  modeName = 'On-Request',
  disabled = false,
  className,
}: ChatInputProps) {
  const [value, setValue] = useState('')

  const handleSend = () => {
    if (value.trim() && onSend) {
      onSend(value.trim())
      setValue('')
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  return (
    <div className={cn('border-t border-border bg-card p-4', className)}>
      <div className="flex rounded-lg bg-muted">
        {/* Left Content */}
        <div className="flex flex-1 flex-col">
          {/* Input Row */}
          <div className="flex h-20 items-start gap-3 p-4">
            <button
              type="button"
              className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-accent text-muted-foreground transition-colors hover:bg-accent/80"
            >
              <Plus className="h-4 w-4" />
            </button>
            <textarea
              value={value}
              onChange={e => setValue(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={placeholder}
              disabled={disabled}
              className={cn(
                'flex-1 resize-none bg-transparent text-sm leading-[1.5] text-foreground',
                'placeholder:text-muted-foreground',
                'focus:outline-none',
                'disabled:cursor-not-allowed disabled:opacity-50'
              )}
              rows={2}
            />
          </div>

          {/* Divider */}
          <div className="h-px bg-border" />

          {/* Control Row */}
          <div className="flex h-10 items-center gap-2 px-4">
            {/* Model Selector */}
            <button
              type="button"
              className="flex h-7 items-center gap-1.5 rounded-md bg-accent px-2.5 transition-colors hover:bg-accent/80"
            >
              <div className="h-3 w-3 rounded-sm bg-muted-foreground" />
              <span className="text-xs text-muted-foreground">{modelName}</span>
              <ChevronDown className="h-3 w-3 text-muted-foreground" />
            </button>

            {/* Mode Selector */}
            <button
              type="button"
              className="flex h-7 items-center gap-1.5 rounded-md bg-accent px-2.5 transition-colors hover:bg-accent/80"
            >
              <div className="h-2 w-2 rounded bg-green-500" />
              <span className="text-xs text-muted-foreground">{modeName}</span>
              <ChevronDown className="h-3 w-3 text-muted-foreground" />
            </button>

            <div className="flex-1" />
          </div>
        </div>

        {/* Right Column */}
        <div className="flex w-[52px] flex-col items-center justify-between pb-3 pr-3 pt-4">
          {/* Send Button */}
          <button
            type="button"
            onClick={handleSend}
            disabled={disabled || !value.trim()}
            className={cn(
              'flex h-8 w-8 items-center justify-center rounded-lg bg-primary',
              'text-primary-foreground transition-colors',
              'hover:bg-primary/90',
              'disabled:cursor-not-allowed disabled:opacity-50'
            )}
          >
            <ArrowUp className="h-5 w-5" strokeWidth={2.5} />
          </button>

          {/* Status Ring */}
          <div className="h-5 w-5 rounded-full border-2 border-green-500" />
        </div>
      </div>
    </div>
  )
}
