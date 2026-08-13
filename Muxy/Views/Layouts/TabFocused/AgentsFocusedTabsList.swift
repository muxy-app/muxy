import SwiftUI

struct AgentsFocusedTabsList: View {
    let project: Project
    let worktree: Worktree

    @Environment(AppState.self) private var appState
    @State private var detectedAgentStore = DetectedAgentStore.shared
    @State private var agentStatusStore = AgentStatusStore.shared
    @State private var dragState = AgentsFocusedTabDragState()

    private struct TabBlock: Identifiable {
        let topLevelTabID: UUID
        let locations: [AgentsFocusedTabSelection.Location]

        var id: UUID { topLevelTabID }
    }

    private var worktreeKey: WorktreeKey {
        WorktreeKey(projectID: project.id, worktreeID: worktree.id)
    }

    private var agentTabs: [AgentsFocusedTabSelection.Location] {
        AgentsFocusedTabSelection.resolve(
            root: appState.workspaceRoots[worktreeKey],
            topLevelTabs: appState.topLevelTabs(for: worktreeKey),
            providerID: providerID
        )
    }

    private var topLevelTabs: [(area: TabArea, tab: TerminalTab)] {
        appState.topLevelTabs(for: worktreeKey)
    }

    private var tabBlocks: [TabBlock] {
        let locationsByTopLevelTabID = Dictionary(grouping: agentTabs, by: \.topLevelTabID)
        return topLevelTabs.compactMap { topLevel in
            guard let locations = locationsByTopLevelTabID[topLevel.tab.id] else { return nil }
            return TabBlock(topLevelTabID: topLevel.tab.id, locations: locations)
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            ForEach(tabBlocks) { block in
                VStack(spacing: 0) {
                    ForEach(block.locations) { location in
                        TabFocusedTabRow(
                            project: project,
                            area: location.area,
                            tab: location.tab,
                            relatedTabs: [location.tab],
                            topLevelTabs: topLevelTabs,
                            active: isActive(location),
                            worktree: worktree
                        )
                    }
                }
                .opacity(dragState.draggedID == block.topLevelTabID ? 0.5 : 1)
                .background {
                    if dragState.draggedID != nil {
                        GeometryReader { geo in
                            Color.clear.preference(
                                key: AgentsFocusedTabRowFramePreferenceKey.self,
                                value: [
                                    block.topLevelTabID: geo.frame(
                                        in: .named(worktreeKey)
                                    ),
                                ]
                            )
                        }
                    }
                }
                .gesture(
                    DragGesture(minimumDistance: 6, coordinateSpace: .named(worktreeKey))
                        .onChanged { value in
                            handleDragChanged(block: block, location: value.location)
                        }
                        .onEnded { _ in
                            handleDragEnded()
                        }
                )
            }
        }
        .coordinateSpace(name: worktreeKey)
        .onPreferenceChange(AgentsFocusedTabRowFramePreferenceKey.self) { frames in
            guard dragState.draggedID != nil else { return }
            dragState.frames = frames
        }
        .onChange(of: tabBlocks.map(\.topLevelTabID)) { _, blockIDs in
            guard let draggedID = dragState.draggedID,
                  !blockIDs.contains(draggedID)
            else { return }
            handleDragEnded()
        }
    }

    private func providerID(for paneID: UUID) -> String? {
        detectedAgentStore.agent(for: paneID) ?? agentStatusStore.activeProviderID(forPane: paneID)
    }

    private func isActive(_ location: AgentsFocusedTabSelection.Location) -> Bool {
        appState.activeProjectID == project.id
            && appState.activeWorktreeID[project.id] == worktree.id
            && appState.focusedAreaID[worktreeKey] == location.area.id
            && location.area.activeTabID == location.tab.id
    }

    private func handleDragChanged(block: TabBlock, location: CGPoint) {
        if dragState.draggedID == nil {
            dragState.draggedID = block.topLevelTabID
            dragState.lastReorderTargetID = nil
        }
        reorderIfNeeded(at: location)
    }

    private func handleDragEnded() {
        withAnimation(.easeInOut(duration: 0.15)) {
            dragState.draggedID = nil
            dragState.frames = [:]
            dragState.lastReorderTargetID = nil
        }
    }

    private func reorderIfNeeded(at location: CGPoint) {
        guard let draggedID = dragState.draggedID else { return }
        var hoveredTargetID: UUID?

        for (id, frame) in dragState.frames where id != draggedID {
            guard frame.contains(location) else { continue }
            hoveredTargetID = id
            guard dragState.lastReorderTargetID != id else { return }

            dragState.lastReorderTargetID = id
            withAnimation(.easeInOut(duration: 0.15)) {
                appState.reorderVisibleTopLevelTabs(
                    for: worktreeKey,
                    moving: draggedID,
                    over: id,
                    visibleTopLevelTabIDs: tabBlocks.map(\.topLevelTabID)
                )
            }
            return
        }

        if hoveredTargetID == nil {
            dragState.lastReorderTargetID = nil
        }
    }
}

private struct AgentsFocusedTabDragState {
    var draggedID: UUID?
    var frames: [UUID: CGRect] = [:]
    var lastReorderTargetID: UUID?
}

private enum AgentsFocusedTabRowFrameTag {}
private typealias AgentsFocusedTabRowFramePreferenceKey = UUIDFramePreferenceKey<AgentsFocusedTabRowFrameTag>
