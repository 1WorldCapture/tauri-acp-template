import { Folder } from 'lucide-react'

interface ChatHeaderProps {
  projectName?: string
  agentName?: string
}

export function ChatHeader({ projectName, agentName }: ChatHeaderProps) {
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
      {/* Header icons placeholder */}
      <div className="flex items-center gap-3" />
    </div>
  )
}
