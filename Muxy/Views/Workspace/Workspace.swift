import SwiftUI

struct TerminalArea: View {
    let project: Project
    let worktreeKey: WorktreeKey
    let isActiveProject: Bool
    @Environment(AppState.self) private var appState
    @Environment(TabDragCoordinator.self) private var dragCoordinator
    @Environment(ProjectGroupStore.self) private var projectGroupStore

    private var workspaceContext: WorkspaceContext {
        projectGroupStore.workspaceContext(for: project)
    }

    var body: some View {
        if let layout = appState.topLevelTabLayouts[worktreeKey] {
            TopLevelWorkspaceNodeView(
                node: layout,
                project: project,
                worktreeKey: worktreeKey,
                isActiveProject: isActiveProject,
                showsTabStrips: !layout.isSingleGroup
            )
            .environment(\.activeWorktreeKey, worktreeKey)
            .environment(\.paneWorkspaceContext, workspaceContext)
            .onPreferenceChange(AreaFramePreferenceKey.self) { frames in
                guard isActiveProject,
                      dragCoordinator.activeDrag?.isTopLevel == false
                else { return }
                dragCoordinator.setAreaFrames(frames, forProject: project.id)
            }
            .onPreferenceChange(TopLevelGroupFramePreferenceKey.self) { frames in
                guard isActiveProject,
                      dragCoordinator.activeDrag?.isTopLevel == true
                else { return }
                dragCoordinator.setGroupFrames(frames, forProject: project.id)
            }
        }
    }
}

private struct TopLevelWorkspaceNodeView: View {
    let node: TopLevelTabNode
    let project: Project
    let worktreeKey: WorktreeKey
    let isActiveProject: Bool
    let showsTabStrips: Bool

    var body: some View {
        Group {
            switch node {
            case let .group(group):
                TopLevelTabGroupContent(
                    group: group,
                    project: project,
                    worktreeKey: worktreeKey,
                    isActiveProject: isActiveProject,
                    showsTabStrip: showsTabStrips
                )
            case let .split(branch):
                TopLevelTabSplitView(
                    branch: branch,
                    project: project,
                    worktreeKey: worktreeKey,
                    isActiveProject: isActiveProject,
                    showsTabStrips: showsTabStrips
                )
            }
        }
        .id(node.id)
    }
}

private struct TopLevelTabSplitView: View {
    let branch: TopLevelTabBranch
    let project: Project
    let worktreeKey: WorktreeKey
    let isActiveProject: Bool
    let showsTabStrips: Bool

    var body: some View {
        GeometryReader { geo in
            let horizontal = branch.direction == .horizontal
            let total = horizontal ? geo.size.width : geo.size.height
            let firstLength = max(0, total * branch.ratio - 0.5)
            let secondLength = max(0, total * (1 - branch.ratio) - 0.5)
            let layout = horizontal
                ? AnyLayout(HStackLayout(spacing: 0))
                : AnyLayout(VStackLayout(spacing: 0))

            layout {
                child(branch.first)
                    .frame(
                        width: horizontal ? firstLength : nil,
                        height: horizontal ? nil : firstLength
                    )

                AnchoredResizeHandle(
                    axis: horizontal ? .horizontal : .vertical,
                    captureAnchor: { branch.ratio },
                    onTranslate: { start, delta in
                        guard total > 0 else { return }
                        branch.ratio = min(max(start + delta / total, 0.15), 0.85)
                    }
                )
                .accessibilityLabel(horizontal ? "Horizontal Tab Group Divider" : "Vertical Tab Group Divider")
                .accessibilityValue("Split ratio: \(Int(branch.ratio * 100))%")
                .accessibilityAdjustableAction { direction in
                    let step: CGFloat = 0.05
                    switch direction {
                    case .increment:
                        branch.ratio = min(branch.ratio + step, 0.85)
                    case .decrement:
                        branch.ratio = max(branch.ratio - step, 0.15)
                    @unknown default:
                        break
                    }
                }

                child(branch.second)
                    .frame(
                        width: horizontal ? secondLength : nil,
                        height: horizontal ? nil : secondLength
                    )
            }
        }
    }

    private func child(_ node: TopLevelTabNode) -> some View {
        TopLevelWorkspaceNodeView(
            node: node,
            project: project,
            worktreeKey: worktreeKey,
            isActiveProject: isActiveProject,
            showsTabStrips: showsTabStrips
        )
    }
}

private struct TopLevelTabGroupContent: View {
    let group: TopLevelTabGroup
    let project: Project
    let worktreeKey: WorktreeKey
    let isActiveProject: Bool
    let showsTabStrip: Bool

    @Environment(AppState.self) private var appState
    @Environment(TabDragCoordinator.self) private var dragCoordinator

    private var focusedAreaID: UUID? {
        guard appState.activeTopLevelTabID(for: worktreeKey) == group.activeTabID else { return nil }
        return appState.focusedAreaID[worktreeKey]
    }

    private var visibleLayout: VisiblePaneNode? {
        appState.visibleLayout(for: worktreeKey, groupID: group.id)
    }

    private var maximizedPane: (area: TabArea, tab: TerminalTab)? {
        guard let maximizedPane = appState.maximizedPanes[worktreeKey],
              maximizedPane.topLevelTabID == group.activeTabID
        else { return nil }
        return visibleLayout?.allPanes().first { $0.area.id == maximizedPane.areaID }
    }

    var body: some View {
        VStack(spacing: 0) {
            if showsTabStrip {
                TopLevelTabGroupStrip(
                    project: project,
                    worktreeKey: worktreeKey,
                    groupID: group.id
                )
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
            }
            if let visibleLayout {
                content(visibleLayout)
            }
        }
        .overlay {
            if dragCoordinator.activeDrag?.isTopLevel == true,
               dragCoordinator.hoveredGroupID == group.id,
               let zone = dragCoordinator.hoveredZone
            {
                DropZoneHighlight(zone: zone)
            }
        }
        .background {
            if dragCoordinator.activeDrag?.isTopLevel == true {
                GeometryReader { geo in
                    Color.clear.preference(
                        key: TopLevelGroupFramePreferenceKey.self,
                        value: [group.id: geo.frame(in: .named(DragCoordinateSpace.mainWindow))]
                    )
                }
            }
        }
    }

    @ViewBuilder
    private func content(_ visibleLayout: VisiblePaneNode) -> some View {
        if let maximizedPane {
            TabAreaView(
                area: maximizedPane.area,
                tab: maximizedPane.tab,
                topLevelGroupID: group.id,
                isFocused: true,
                isActiveProject: isActiveProject,
                projectID: project.id,
                onFocus: {
                    selectPane(areaID: maximizedPane.area.id, tabID: maximizedPane.tab.id)
                },
                onForceCloseTab: {
                    appState.forceCloseTab(
                        maximizedPane.tab.id,
                        areaID: maximizedPane.area.id,
                        projectID: project.id
                    )
                },
                onDropAction: handleDrop
            )
            .id(maximizedPane.tab.id)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .strokeBorder(MuxyTheme.border, lineWidth: 1)
            )
            .shadow(color: Color.black.opacity(0.35), radius: 24, x: 0, y: 8)
            .padding(16)
        } else {
            PaneNode(
                node: visibleLayout,
                topLevelGroupID: group.id,
                focusedAreaID: focusedAreaID,
                isActiveProject: isActiveProject,
                projectID: project.id,
                onSelectPane: selectPane,
                onForceCloseTab: { areaID, tabID in
                    appState.forceCloseTab(tabID, areaID: areaID, projectID: project.id)
                },
                onDropAction: handleDrop
            )
        }
    }

    private func selectPane(areaID: UUID, tabID: UUID) {
        appState.dispatch(.selectTab(projectID: project.id, areaID: areaID, tabID: tabID))
    }

    private func handleDrop(_ result: TabDragCoordinator.DropResult) {
        appState.dispatch(result.action(projectID: project.id))
    }
}

private enum TopLevelGroupFrameTag {}
private typealias TopLevelGroupFramePreferenceKey = UUIDFramePreferenceKey<TopLevelGroupFrameTag>
