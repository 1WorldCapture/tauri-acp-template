import { cn } from '@/lib/utils'
import { useUIStore } from '@/store/ui-store'
import { ChatArea, UserMessage, AIMessage } from '@/components/chat'
import { useProjectsList, getProjectName } from '@/services/projects'
import { useAgentsList, getAgentDisplayName } from '@/services/agents'

interface MainWindowContentProps {
  children?: React.ReactNode
  className?: string
}

export function MainWindowContent({
  children,
  className,
}: MainWindowContentProps) {
  const selectedAgentId = useUIStore(state => state.selectedAgentId)
  const selectedProjectId = useUIStore(state => state.selectedProjectId)

  // Get project data
  const { data: projects } = useProjectsList()
  const selectedProject = projects?.find(
    p => p.workspaceId === selectedProjectId
  )
  const projectName = selectedProject
    ? getProjectName(selectedProject.rootDir)
    : undefined

  // Get agent data
  const { data: agents } = useAgentsList(selectedProjectId)
  const selectedAgent = agents?.find(a => a.agentId === selectedAgentId)
  const agentName = selectedAgent
    ? getAgentDisplayName(selectedAgent)
    : undefined

  // Demo messages for the chat UI
  const demoMessages = (
    <>
      <UserMessage>
        <div className="flex flex-col gap-3">
          <p>
            当前这个项目是基于 React 加上 Tauri
            技术来实现的，所以它的界面其实都是前端的 Web 页面。
          </p>
          <p>
            如果你看我发给你的截图就能发现，原生的界面文字按理说应该是不可选的。但因为我们这个项目是用前端做的，可能在可选择性的设置上没有处理好，导致界面上的东西都可以被选中，这样其实是不对的。
          </p>
          <p>所以请你调研并解决一下这个问题。我想达到的效果是：</p>
          <div className="flex flex-col gap-1">
            <p>1. 让它和原生界面保持一致</p>
            <p>2. 界面上的标题等元素设置为不可选择</p>
            <p>3. 用户不可能把界面上的文本元素给选中并提取出来</p>
          </div>
        </div>
      </UserMessage>

      <AIMessage>
        <div className="flex flex-col gap-3">
          <p>
            已通过全局样式把非输入控件的文本选择能力关掉，使 UI
            行为更接近原生（标题/列表/按钮/提示等都无法被拖拽选中）。
          </p>
          <div className="flex flex-col gap-2">
            <p>
              • 变更在 src/styles/base.css:279: 对 .app 统一设置 user-select:
              none
            </p>
            <p>
              • 同时在 src/styles/base.css:286 仅对 input/textarea 放开为
              user-select: text
            </p>
          </div>
          <p>
            我也跑了 npm run lint / npm run typecheck; lint
            只有一个与本次改动无关的既有 warning。
          </p>
        </div>
      </AIMessage>

      <UserMessage>
        <div className="flex flex-col gap-3">
          <p>
            这样的话，如果用户想要复制聊天区域的文本，是不是也不能选中了？那是不对的。
          </p>
          <p>请你再考虑一下解决方案。</p>
        </div>
      </UserMessage>

      <AIMessage>
        <div className="flex flex-col gap-3">
          <p>
            你说得对：不应该一刀切把整个界面都设成不可选，聊天/内容区需要能复制。
          </p>
          <p>我把策略改成了默认不可选 + 内容区白名单可选:</p>
          <div className="flex flex-col gap-2">
            <p>• 全局 user-select: none 保持不变</p>
            <p>• 聊天消息区域添加 user-select: text 白名单</p>
            <p>• 代码块、输入框等同理保持可选</p>
          </div>
        </div>
      </AIMessage>
    </>
  )

  // Show chat area only when both project and agent are selected
  if (selectedProjectId && selectedAgentId) {
    return (
      <div className={cn('flex h-full flex-col', className)}>
        <ChatArea
          projectName={projectName}
          agentName={agentName}
          onSendMessage={message => {
            console.log('Send message:', message)
          }}
        >
          {demoMessages}
        </ChatArea>
      </div>
    )
  }

  // Default: show placeholder or children
  return (
    <div className={cn('flex h-full flex-col bg-background', className)}>
      {children || (
        <div className="flex flex-1 flex-col items-center justify-center">
          <h1 className="text-4xl font-bold text-foreground">
            {selectedProjectId
              ? 'Select an agent to start chatting'
              : 'Select a project to get started'}
          </h1>
        </div>
      )}
    </div>
  )
}
