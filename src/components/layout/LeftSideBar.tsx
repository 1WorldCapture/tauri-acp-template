import { cn } from '@/lib/utils'
import { ProjectsSidebar } from '@/components/projects/ProjectsSidebar'

interface LeftSideBarProps {
  children?: React.ReactNode
  className?: string
}

export function LeftSideBar({ children, className }: LeftSideBarProps) {
  return (
    <div
      className={cn('flex h-full flex-col border-r bg-background', className)}
    >
      {children ?? <ProjectsSidebar />}
    </div>
  )
}
