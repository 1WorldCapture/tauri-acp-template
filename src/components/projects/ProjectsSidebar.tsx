import { Plus, Trash2 } from 'lucide-react'
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
import { cn } from '@/lib/utils'
import { showContextMenu } from '@/lib/context-menu'
import { useUIStore } from '@/store/ui-store'
import {
  useProjectsList,
  useCreateProject,
  useDeleteProject,
  getProjectName,
} from '@/services/projects'

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
  name: string
  isSelected: boolean
  onClick: () => void
  onContextMenu: (e: React.MouseEvent) => void
  onDelete: () => void
}

function ProjectCard({
  name,
  isSelected,
  onClick,
  onContextMenu,
  onDelete,
}: ProjectCardProps) {
  return (
    <button
      type="button"
      className={cn(
        'group relative w-full cursor-pointer rounded-md px-3 py-2 text-left transition-colors',
        isSelected ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50'
      )}
      onClick={onClick}
      onContextMenu={onContextMenu}
    >
      <span className="text-[15px] font-semibold">{name}</span>

      {/* Delete button on hover */}
      <Button
        variant="ghost"
        size="icon-sm"
        className="absolute top-1 right-1 size-6 opacity-0 transition-opacity group-hover:opacity-100"
        onClick={e => {
          e.stopPropagation()
          onDelete()
        }}
      >
        <Trash2 className="size-3.5 text-muted-foreground hover:text-destructive" />
      </Button>
    </button>
  )
}
