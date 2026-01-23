import { Plus, Trash2, Cloud, Terminal } from 'lucide-react'
import { open } from '@tauri-apps/plugin-dialog'

import { Button } from '@/components/ui/button'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'
import { showContextMenu } from '@/lib/context-menu'
import { useUIStore } from '@/store/ui-store'
import {
  useProjectsList,
  useCreateProject,
  useDeleteProject,
  getProjectName,
} from '@/services/projects'
import {
  useAgentsList,
  useCreateAgent,
  getAgentDisplayName,
} from '@/services/agents'

export function ProjectsSidebar() {
  const { data: projects, isLoading, isError } = useProjectsList()
  const createProject = useCreateProject()
  const deleteProject = useDeleteProject()

  const selectedProjectId = useUIStore(state => state.selectedProjectId)
  const setSelectedProjectId = useUIStore(state => state.setSelectedProjectId)
  const projectPendingDeleteId = useUIStore(
    state => state.projectPendingDeleteId
  )
  const setProjectPendingDeleteId = useUIStore(
    state => state.setProjectPendingDeleteId
  )

  const pendingDeleteProject = projects?.find(
    p => p.workspaceId === projectPendingDeleteId
  )

  const handleCreateProject = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Project Folder',
    })

    if (selected && typeof selected === 'string') {
      try {
        const result = await createProject.mutateAsync(selected)
        setSelectedProjectId(result.workspaceId)
      } catch {
        // Error already handled by onError callback in useCreateProject
      }
    }
  }

  const handleContextMenu = (e: React.MouseEvent, workspaceId: string) => {
    e.preventDefault()
    showContextMenu([
      {
        id: 'delete',
        label: 'Delete Project',
        action: () => setProjectPendingDeleteId(workspaceId),
      },
    ])
  }

  const handleDeleteConfirm = async () => {
    if (projectPendingDeleteId) {
      try {
        await deleteProject.mutateAsync(projectPendingDeleteId)
        if (selectedProjectId === projectPendingDeleteId) {
          setSelectedProjectId(null)
        }
      } catch {
        // Error already handled by onError callback in useDeleteProject
      } finally {
        setProjectPendingDeleteId(null)
      }
    }
  }

  const handleDeleteCancel = () => {
    setProjectPendingDeleteId(null)
  }

  return (
    <div className="flex h-full flex-col p-5">
      {/* Header */}
      <div className="mb-6 flex items-center justify-between pb-2">
        <span className="text-sm font-semibold text-muted-foreground">
          Projects
        </span>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={handleCreateProject}
          disabled={createProject.isPending}
          className="size-6"
        >
          <Plus className="size-4" />
        </Button>
      </div>

      {/* Projects List */}
      <div className="flex flex-1 flex-col gap-6 overflow-y-auto">
        {isLoading ? (
          <div className="text-sm text-muted-foreground">Loading...</div>
        ) : isError ? (
          <div className="text-sm text-destructive">
            Failed to load projects. Please try again.
          </div>
        ) : projects?.length === 0 ? (
          <div className="text-sm text-muted-foreground">
            No projects yet. Click + to add one.
          </div>
        ) : (
          projects?.map(project => (
            <ProjectCard
              key={project.workspaceId}
              workspaceId={project.workspaceId}
              name={getProjectName(project.rootDir)}
              isSelected={selectedProjectId === project.workspaceId}
              onClick={() => setSelectedProjectId(project.workspaceId)}
              onContextMenu={e => handleContextMenu(e, project.workspaceId)}
              onDelete={() => setProjectPendingDeleteId(project.workspaceId)}
            />
          ))
        )}
      </div>

      {/* Delete Confirmation Dialog */}
      <AlertDialog
        open={!!projectPendingDeleteId}
        onOpenChange={open => !open && handleDeleteCancel()}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Project</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete &quot;
              {pendingDeleteProject
                ? getProjectName(pendingDeleteProject.rootDir)
                : ''}
              &quot;? This will remove the project from the list but will not
              delete any files from your disk.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={handleDeleteCancel}>
              Cancel
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDeleteConfirm}
              disabled={deleteProject.isPending}
              className="bg-destructive text-white hover:bg-destructive/90"
            >
              {deleteProject.isPending ? 'Deleting...' : 'Delete'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}

interface ProjectCardProps {
  workspaceId: string
  name: string
  isSelected: boolean
  onClick: () => void
  onContextMenu: (e: React.MouseEvent) => void
  onDelete: () => void
}

function ProjectCard({
  workspaceId,
  name,
  isSelected,
  onClick,
  onContextMenu,
  onDelete,
}: ProjectCardProps) {
  const createAgent = useCreateAgent()
  // Only load agents for selected project to avoid N+1 queries
  const { data: agents } = useAgentsList(isSelected ? workspaceId : null)

  const selectedAgentId = useUIStore(state => state.selectedAgentId)
  const selectAgent = useUIStore(state => state.selectAgent)

  const handleCreateAgent = (pluginId: string) => {
    createAgent.mutate(
      { workspaceId, pluginId },
      {
        onSuccess: agent => {
          // Set both project and agent IDs together
          selectAgent(workspaceId, agent.agentId)
        },
      }
    )
  }

  return (
    <div
      className={cn(
        'group rounded-md px-3 py-2 transition-colors',
        isSelected ? 'bg-accent' : 'hover:bg-accent/50'
      )}
    >
      {/* Project Header Row */}
      <div className="flex w-full items-center justify-between">
        <button
          type="button"
          className="flex-1 text-left"
          onClick={onClick}
          onContextMenu={onContextMenu}
        >
          <span
            className={cn(
              'text-[15px] font-semibold',
              isSelected ? 'text-accent-foreground' : ''
            )}
          >
            {name}
          </span>
        </button>

        {/* Actions on hover */}
        <div className="flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          {/* Add Agent Dropdown */}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon-sm" className="size-6">
                <Plus className="size-3.5 text-muted-foreground" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" sideOffset={4}>
              <DropdownMenuItem
                onClick={() => handleCreateAgent('claude-code')}
              >
                <Cloud className="size-4" />
                <span>New Claude Agent</span>
              </DropdownMenuItem>
              <DropdownMenuItem onClick={() => handleCreateAgent('codex')}>
                <Terminal className="size-4" />
                <span>New Codex Agent</span>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          {/* Delete button */}
          <Button
            variant="ghost"
            size="icon-sm"
            className="size-6"
            onClick={onDelete}
          >
            <Trash2 className="size-3.5 text-muted-foreground hover:text-destructive" />
          </Button>
        </div>
      </div>

      {/* Agents List */}
      {agents && agents.length > 0 && (
        <div className="mt-2 flex flex-col gap-1">
          {agents.map(agent => (
            <button
              key={agent.agentId}
              type="button"
              className={cn(
                'flex items-center gap-2 rounded px-2 py-1 text-left text-sm transition-colors',
                selectedAgentId === agent.agentId
                  ? 'bg-primary/10 text-primary'
                  : 'text-muted-foreground hover:bg-accent/50 hover:text-foreground'
              )}
              onClick={e => {
                e.stopPropagation()
                // Set both project and agent IDs together
                selectAgent(workspaceId, agent.agentId)
              }}
            >
              {agent.pluginId === 'claude-code' ? (
                <Cloud className="size-3.5" />
              ) : (
                <Terminal className="size-3.5" />
              )}
              <span className="truncate">{getAgentDisplayName(agent)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
