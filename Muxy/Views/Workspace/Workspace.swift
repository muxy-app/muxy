import SwiftUI

struct TerminalArea: View {
    let project: Project
    let worktreeKey: WorktreeKey
    let isActiveProject: Bool
    @Environment(AppState.self) private var appState
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @Environment(\.openWindow) private var openWindow

    private var root: SplitNode? {
        appState.workspaceRoots[worktreeKey]
    }

    private var focusedAreaID: UUID? {
        appState.focusedAreaID[worktreeKey]
    }

    private var rootIsTabArea: Bool {
        guard let root else { return false }
        if case .tabArea = root { return true }
        return false
    }

    private var maximizedArea: TabArea? {
        guard let areaID = appState.maximizedAreaID[worktreeKey] else { return nil }
        return root?.findArea(id: areaID)
    }

    var body: some View {
        if let root {
            workspaceContent(root: root)
                .environment(\.activeWorktreeKey, worktreeKey)
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
                onToggleMaximize: {
                    appState.toggleMaximize(areaID: area.id, for: project.id)
                },
                onSelectTab: { tabID in
                    appState.dispatch(.selectTab(projectID: project.id, areaID: area.id, tabID: tabID))
                },
                onCreateTab: {
                    appState.dispatch(.createTab(projectID: project.id, areaID: area.id))
                },
                onCreateVCSTab: {
                    VCSDisplayMode.current.route(
                        tab: { appState.dispatch(.createVCSTab(projectID: project.id, areaID: area.id)) },
                        window: { openWindow(id: "vcs") },
                        attached: { NotificationCenter.default.post(name: .toggleAttachedVCS, object: nil) }
                    )
                },
                onCloseTab: { tabID in
                    appState.closeTab(tabID, areaID: area.id, projectID: project.id)
                },
                onForceCloseTab: { tabID in
                    appState.forceCloseTab(tabID, areaID: area.id, projectID: project.id)
                },
                onSplit: { dir in
                    appState.dispatch(.splitArea(.init(
                        projectID: project.id,
                        areaID: area.id,
                        direction: dir,
                        position: .second
                    )))
                },
                onDropAction: { result in
                    appState.dispatch(result.action(projectID: project.id))
                }
            )
            .padding(16)
        case nil:
            PaneNode(
                node: root,
                focusedAreaID: focusedAreaID,
                isActiveProject: isActiveProject,
                showTabStrip: !rootIsTabArea,
                showVCSButton: false,
                projectID: project.id,
                shortcutOffsets: appState.shortcutOffsets(for: project.id),
                onFocusArea: { areaID in
                    appState.dispatch(.focusArea(projectID: project.id, areaID: areaID))
                },
                onSelectTab: { areaID, tabID in
                    appState.dispatch(.selectTab(projectID: project.id, areaID: areaID, tabID: tabID))
                },
                onCreateTab: { areaID in
                    appState.dispatch(.createTab(projectID: project.id, areaID: areaID))
                },
                onCreateVCSTab: { areaID in
                    VCSDisplayMode.current.route(
                        tab: { appState.dispatch(.createVCSTab(projectID: project.id, areaID: areaID)) },
                        window: { openWindow(id: "vcs") },
                        attached: { NotificationCenter.default.post(name: .toggleAttachedVCS, object: nil) }
                    )
                },
                onCloseTab: { areaID, tabID in
                    appState.closeTab(tabID, areaID: areaID, projectID: project.id)
                },
                onForceCloseTab: { areaID, tabID in
                    appState.forceCloseTab(tabID, areaID: areaID, projectID: project.id)
                },
                onSplit: { areaID, dir in
                    appState.dispatch(.splitArea(.init(
                        projectID: project.id,
                        areaID: areaID,
                        direction: dir,
                        position: .second
                    )))
                },
                onCloseArea: { areaID in
                    appState.dispatch(.closeArea(projectID: project.id, areaID: areaID))
                },
                onDropAction: { result in
                    appState.dispatch(result.action(projectID: project.id))
                },
                showMaximizeButton: !rootIsTabArea,
                onToggleMaximize: { areaID in
                    appState.toggleMaximize(areaID: areaID, for: project.id)
                }
            )
        }
    }
}

private struct MaximizedAreaView: View {
    let area: TabArea
    let isActiveProject: Bool
    let projectID: UUID
    let onToggleMaximize: () -> Void
    let onSelectTab: (UUID) -> Void
    let onCreateTab: () -> Void
    let onCreateVCSTab: () -> Void
    let onCloseTab: (UUID) -> Void
    let onForceCloseTab: (UUID) -> Void
    let onSplit: (SplitDirection) -> Void
    let onDropAction: (TabDragCoordinator.DropResult) -> Void

    var body: some View {
        TabAreaView(
            area: area,
            isFocused: true,
            isActiveProject: isActiveProject,
            showTabStrip: true,
            showVCSButton: false,
            projectID: projectID,
            shortcutIndexOffset: 0,
            onFocus: {},
            onSelectTab: onSelectTab,
            onCreateTab: onCreateTab,
            onCreateVCSTab: onCreateVCSTab,
            onCloseTab: onCloseTab,
            onForceCloseTab: onForceCloseTab,
            onSplit: onSplit,
            onDropAction: onDropAction,
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
