import { useState } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ToolCallEntry } from '@/store/chat-store'

interface ToolCallCardProps {
  toolCall: ToolCallEntry
  defaultOpen?: boolean
  className?: string
}

/**
 * Get status indicator dot classes based on tool call status.
 * - Yellow pulsing: pending, running, in_progress
 * - Green static: completed, succeeded, done
 * - Red static: failed, error, errored
 */
function getStatusDotClass(status: unknown): string {
  const s = String(status ?? '').toLowerCase()

  // Running/pending states - yellow with pulse
  if (
    s === 'pending' ||
    s === 'running' ||
    s === 'in_progress' ||
    s === 'inprogress' ||
    s === 'tool_use' ||
    s === ''
  ) {
    return 'bg-yellow-500 animate-pulse'
  }

  // Completed/success states - green static
  if (
    s === 'completed' ||
    s === 'succeeded' ||
    s === 'success' ||
    s === 'done'
  ) {
    return 'bg-green-500'
  }

  // Failed/error states - red static
  if (
    s === 'failed' ||
    s === 'error' ||
    s === 'errored' ||
    s === 'cancelled' ||
    s === 'canceled'
  ) {
    return 'bg-red-500'
  }

  // Unknown status - muted
  return 'bg-muted-foreground/40'
}

/**
 * Format a value as JSON for display.
 */
function formatJson(value: unknown): string {
  if (value === undefined || value === null) return ''
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

/**
 * ToolCallCard displays a tool call as an interactive card with:
 * - Status indicator dot (yellow pulsing, green, or red)
 * - Title and kind
 * - Collapsible result/error section
 */
export function ToolCallCard({
  toolCall,
  defaultOpen = false,
  className,
}: ToolCallCardProps) {
  const [open, setOpen] = useState(defaultOpen)

  // Handle click to toggle, but allow text selection
  const handleClick = () => {
    // Don't toggle if user is selecting text
    const selection = window.getSelection()
    if (selection && selection.type === 'Range') {
      return
    }
    setOpen(v => !v)
  }

  // Handle keyboard toggle
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      setOpen(v => !v)
    }
  }

  // Determine what to show in expanded view
  const hasError = !!toolCall.error
  const hasResult = toolCall.result !== undefined && toolCall.result !== null
  const hasRaw = toolCall.raw !== undefined && toolCall.raw !== null
  const hasExpandableContent = hasError || hasResult || hasRaw

  // Build display title
  const displayTitle = toolCall.title || toolCall.kind || 'Tool Call'
  const shortId = toolCall.toolCallId?.slice(-8)

  return (
    <div
      role="button"
      tabIndex={0}
      aria-expanded={open}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      className={cn(
        'group w-full max-w-[615px] cursor-pointer rounded-xl border bg-card px-4 py-3',
        'transition-colors hover:bg-accent/50',
        'focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2',
        className
      )}
    >
      {/* Header row */}
      <div className="flex items-center gap-3">
        {/* Status indicator dot */}
        <span
          className={cn(
            'h-2.5 w-2.5 shrink-0 rounded-full ring-1 ring-border',
            getStatusDotClass(toolCall.status)
          )}
          aria-label={`Status: ${toolCall.status || 'pending'}`}
        />

        {/* Title and kind */}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-medium text-sm">{displayTitle}</span>
            {toolCall.kind && toolCall.title && (
              <span className="shrink-0 text-xs text-muted-foreground">
                {toolCall.kind}
              </span>
            )}
          </div>
          {shortId && (
            <div className="text-xs text-muted-foreground/60">
              id: ...{shortId}
            </div>
          )}
        </div>

        {/* Expand/collapse indicator */}
        {hasExpandableContent && (
          <span className="shrink-0 text-muted-foreground transition-transform">
            {open ? (
              <ChevronDown className="h-4 w-4" />
            ) : (
              <ChevronRight className="h-4 w-4" />
            )}
          </span>
        )}
      </div>

      {/* Expanded content */}
      {open && hasExpandableContent && (
        <div className="mt-3 border-t pt-3">
          {/* Error display */}
          {hasError && (
            <div className="mb-2 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
              <div className="font-medium">Error</div>
              <pre className="mt-1 max-h-40 select-text overflow-auto whitespace-pre-wrap font-mono text-xs">
                {toolCall.error}
              </pre>
            </div>
          )}

          {/* Result display */}
          {hasResult && (
            <div className="mb-2">
              <div className="mb-1 text-xs font-medium text-muted-foreground">
                Result
              </div>
              <pre className="max-h-80 select-text overflow-auto rounded-md bg-muted/50 px-3 py-2 font-mono text-xs">
                {formatJson(toolCall.result)}
              </pre>
            </div>
          )}

          {/* Raw payload fallback (only if no result) */}
          {!hasResult && hasRaw && (
            <div>
              <div className="mb-1 text-xs font-medium text-muted-foreground">
                Raw Payload
              </div>
              <pre className="max-h-60 select-text overflow-auto rounded-md bg-muted/50 px-3 py-2 font-mono text-xs">
                {formatJson(toolCall.raw)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
