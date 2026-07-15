import SwiftUI

struct TerminalArea: View {
    let project: Project
    let worktreeKey: WorktreeKey
    let isActiveProject: Bool
    @Environment(AppState.self) private var appState
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    private var root: SplitNode? {
        appState.workspaceRoots[worktreeKey]
    }

    private var workspaceContext: WorkspaceContext {
        projectGroupStore.workspaceContext(for: project)
    }

    private var focusedAreaID: UUID? {
        appState.focusedAreaID[worktreeKey]
    }

    private var rootIsTabArea: Bool {
        guard let root else { return false }
        if case .tabArea = root { return true }
        return false
    }

    private var actions: WorkspaceViewActions {
        .local(projectID: project.id, appState: appState)
    }

    private var maximizedArea: TabArea? {
        guard let areaID = appState.maximizedAreaID[worktreeKey] else { return nil }
        return root?.findArea(id: areaID)
    }

    var body: some View {
        if let root {
            workspaceContent(root: root)
                .environment(\.activeWorktreeKey, worktreeKey)
                .environment(\.paneWorkspaceContext, workspaceContext)
                .onPreferenceChange(AreaFramePreferenceKey.self) { frames in
                    guard isActiveProject, dragCoordinator.activeDrag != nil else { return }
                    dragCoordinator.setAreaFrames(frames, forProject: project.id)
                }
        }
    }

    @ViewBuilder
    private func workspaceContent(root: SplitNode) -> some View {
        switch maximizedArea {
        case let area?:
            MaximizedAreaView(
                area: area,
                isActiveProject: isActiveProject,
                projectID: project.id,
                actions: actions,
                onToggleMaximize: {
                    appState.toggleMaximize(areaID: area.id, for: project.id)
                }
            )
            .padding(16)
        case nil:
            PaneNode(
                node: root,
                focusedAreaID: focusedAreaID,
                isActiveProject: isActiveProject,
                showTabStrip: !rootIsTabArea,
                shortcutOffsets: appState.shortcutOffsets(for: project.id),
                actions: actions,
                showMaximizeButton: !rootIsTabArea,
                onToggleMaximize: { areaID in
                    appState.toggleMaximize(areaID: areaID, for: project.id)
                }
            )
        }
    }
}

struct RemoteWorkspaceArea: View {
    let project: Project
    let presentation: RemoteWorkspacePresentation
    let actions: WorkspaceViewActions

    @State private var maximizedAreaID: UUID?

    private var rootIsTabArea: Bool {
        if case .tabArea = presentation.root { return true }
        return false
    }

    private var maximizedArea: TabArea? {
        guard let maximizedAreaID else { return nil }
        return presentation.root.findArea(id: maximizedAreaID)
    }

    private var shortcutOffsets: [UUID: Int] {
        var offset = 0
        var result: [UUID: Int] = [:]
        for area in presentation.root.allAreas() {
            result[area.id] = offset
            offset += area.tabs.count
        }
        return result
    }

    var body: some View {
        if let area = maximizedArea {
            MaximizedAreaView(
                area: area,
                isActiveProject: true,
                projectID: project.id,
                actions: actions,
                onToggleMaximize: { maximizedAreaID = nil }
            )
            .padding(16)
        } else {
            PaneNode(
                node: presentation.root,
                focusedAreaID: presentation.focusedAreaID,
                isActiveProject: true,
                showTabStrip: !rootIsTabArea,
                shortcutOffsets: shortcutOffsets,
                actions: actions,
                showMaximizeButton: !rootIsTabArea,
                onToggleMaximize: { maximizedAreaID = $0 }
            )
        }
    }
}

struct MaximizedAreaView: View {
    let area: TabArea
    let isActiveProject: Bool
    let projectID: UUID
    let actions: WorkspaceViewActions
    let onToggleMaximize: () -> Void

    var body: some View {
        TabAreaView(
            area: area,
            isFocused: true,
            isActiveProject: isActiveProject,
            showTabStrip: true,
            projectID: projectID,
            shortcutIndexOffset: 0,
            actions: actions,
            showMaximizeButton: true,
            isMaximized: true,
            onToggleMaximize: onToggleMaximize
        )
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(MuxyTheme.border, lineWidth: 1)
        )
        .shadow(color: Color.black.opacity(0.35), radius: 24, x: 0, y: 8)
    }
}
