import { Folder, Loader2 } from 'lucide-react'
import type { AgentStatusLike } from '@/store/chat-store'

interface ChatHeaderProps {
  projectName?: string
  agentName?: string
  agentStatus?: AgentStatusLike
}

export function ChatHeader({
  projectName,
  agentName,
  agentStatus,
}: ChatHeaderProps) {
  // Render status indicator
  const renderStatus = () => {
    if (!agentStatus) return null

    switch (agentStatus.type) {
      case 'starting':
        return (
          <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
            <Loader2 className="h-3 w-3 animate-spin" />
            Starting...
          </span>
        )
      case 'running':
        return (
          <span className="flex items-center gap-1.5 text-xs text-green-500">
            <span className="h-2 w-2 rounded-full bg-green-500" />
            Running
          </span>
        )
      case 'errored':
        return (
          <span className="flex items-center gap-1.5 text-xs text-destructive">
            <span className="h-2 w-2 rounded-full bg-destructive" />
            Error
          </span>
        )
      case 'stopped':
      default:
        return null
    }
  }

  return (
    <div className="flex h-12 items-center gap-3 border-b border-border bg-card px-4">
      <Folder className="h-4 w-4 text-muted-foreground" />
      {projectName ? (
        <span className="text-[13px] text-foreground">{projectName}</span>
      ) : (
        <span className="text-[13px] text-muted-foreground">No project</span>
      )}
      {agentName && (
        <>
          <span className="text-sm text-muted-foreground">›</span>
          <span className="text-[13px] text-muted-foreground">{agentName}</span>
        </>
      )}
      <div className="flex-1" />
      {/* Status indicator */}
      {renderStatus()}
      {/* Header icons placeholder */}
      <div className="flex items-center gap-3" />
    </div>
  )
}
