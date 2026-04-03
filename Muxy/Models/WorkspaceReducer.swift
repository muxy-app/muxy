import Foundation

@MainActor
struct WorkspaceState {
    var activeProjectID: UUID?
    var workspaceRoots: [UUID: SplitNode]
    var focusedAreaID: [UUID: UUID]
    var focusHistory: [UUID: [UUID]]
}

@MainActor
struct WorkspaceSideEffects {
    var paneIDsToRemove: [UUID] = []
}

@MainActor
enum WorkspaceReducer {
    static func reduce(action: AppState.Action, state: inout WorkspaceState) -> WorkspaceSideEffects {
        var effects = WorkspaceSideEffects()

        switch action {
        case let .selectProject(projectID, projectPath):
            state.activeProjectID = projectID
            ensureWorkspaceExists(projectID: projectID, projectPath: projectPath, state: &state)

        case let .removeProject(projectID):
            removeProject(projectID: projectID, state: &state, effects: &effects)

        case let .createTab(projectID, areaID):
            guard let area = resolveArea(projectID: projectID, areaID: areaID, state: state) else { break }
            focusArea(area.id, projectID: projectID, state: &state)
            area.createTab()

        case let .closeTab(projectID, areaID, tabID):
            closeTab(tabID, areaID: areaID, projectID: projectID, state: &state, effects: &effects)

        case let .selectTab(projectID, areaID, tabID):
            guard let area = resolveArea(projectID: projectID, areaID: areaID, state: state) else { break }
            focusArea(area.id, projectID: projectID, state: &state)
            area.selectTab(tabID)

        case let .selectTabByIndex(projectID, areaID, index):
            guard let area = resolveArea(projectID: projectID, areaID: areaID, state: state) else { break }
            focusArea(area.id, projectID: projectID, state: &state)
            area.selectTabByIndex(index)

        case let .selectNextTab(projectID):
            guard let area = resolveArea(projectID: projectID, areaID: nil, state: state) else { break }
            area.selectNextTab()

        case let .selectPreviousTab(projectID):
            guard let area = resolveArea(projectID: projectID, areaID: nil, state: state) else { break }
            area.selectPreviousTab()

        case let .splitArea(projectID, areaID, direction, projectPath):
            splitArea(areaID, direction: direction, projectID: projectID, projectPath: projectPath, state: &state)

        case let .closeArea(projectID, areaID):
            closeArea(areaID, projectID: projectID, state: &state, effects: &effects)

        case let .focusArea(projectID, areaID):
            focusArea(areaID, projectID: projectID, state: &state)

        case let .selectNextProject(projects):
            cycleProject(projects: projects, forward: true, state: &state)

        case let .selectPreviousProject(projects):
            cycleProject(projects: projects, forward: false, state: &state)
        }

        return effects
    }

    private static func splitArea(
        _ areaID: UUID,
        direction: SplitDirection,
        projectID: UUID,
        projectPath: String,
        state: inout WorkspaceState
    ) {
        guard let root = state.workspaceRoots[projectID] else { return }
        let (newRoot, newAreaID) = root.splitting(areaID: areaID, direction: direction, projectPath: projectPath)
        state.workspaceRoots[projectID] = newRoot
        guard let newAreaID else { return }
        if let current = state.focusedAreaID[projectID] {
            state.focusHistory[projectID, default: []].append(current)
        }
        state.focusedAreaID[projectID] = newAreaID
    }

    private static func closeArea(
        _ areaID: UUID,
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard let root = state.workspaceRoots[projectID] else { return }
        if let area = root.findArea(id: areaID) {
            effects.paneIDsToRemove.append(contentsOf: area.tabs.map(\.pane.id))
        }
        guard let newRoot = root.removing(areaID: areaID) else {
            state.workspaceRoots.removeValue(forKey: projectID)
            state.focusedAreaID.removeValue(forKey: projectID)
            state.focusHistory.removeValue(forKey: projectID)
            state.activeProjectID = nil
            return
        }

        state.workspaceRoots[projectID] = newRoot
        state.focusHistory[projectID]?.removeAll { $0 == areaID }
        guard state.focusedAreaID[projectID] == areaID else { return }

        let remaining = newRoot.allAreas()
        let previousID = popFocusHistory(projectID: projectID, validAreas: remaining, state: &state)
        state.focusedAreaID[projectID] = previousID ?? remaining.first?.id
    }

    private static func closeTab(
        _ tabID: UUID,
        areaID: UUID,
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard let root = state.workspaceRoots[projectID],
              let area = root.findArea(id: areaID)
        else { return }

        let areaCount = root.allAreas().count
        if area.tabs.count <= 1, areaCount > 1 {
            closeArea(areaID, projectID: projectID, state: &state, effects: &effects)
            return
        }

        if let paneID = area.closeTab(tabID) {
            effects.paneIDsToRemove.append(paneID)
        }

        guard area.tabs.isEmpty else { return }
        state.workspaceRoots.removeValue(forKey: projectID)
        state.focusedAreaID.removeValue(forKey: projectID)
        state.focusHistory.removeValue(forKey: projectID)
        state.activeProjectID = nil
    }

    private static func focusArea(_ areaID: UUID, projectID: UUID, state: inout WorkspaceState) {
        if let current = state.focusedAreaID[projectID], current != areaID {
            state.focusHistory[projectID, default: []].append(current)
        }
        state.focusedAreaID[projectID] = areaID
    }

    private static func cycleProject(projects: [Project], forward: Bool, state: inout WorkspaceState) {
        guard projects.count > 1,
              let currentID = state.activeProjectID,
              let index = projects.firstIndex(where: { $0.id == currentID })
        else { return }
        let next = forward ? (index + 1) % projects.count : (index - 1 + projects.count) % projects.count
        let project = projects[next]
        state.activeProjectID = project.id
        ensureWorkspaceExists(projectID: project.id, projectPath: project.path, state: &state)
    }

    private static func removeProject(
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        if let root = state.workspaceRoots[projectID] {
            let paneIDs = root.allAreas().flatMap { $0.tabs.map(\.pane.id) }
            effects.paneIDsToRemove.append(contentsOf: paneIDs)
        }
        state.workspaceRoots.removeValue(forKey: projectID)
        state.focusedAreaID.removeValue(forKey: projectID)
        state.focusHistory.removeValue(forKey: projectID)
        if state.activeProjectID == projectID {
            state.activeProjectID = nil
        }
    }

    private static func ensureWorkspaceExists(projectID: UUID, projectPath: String, state: inout WorkspaceState) {
        guard state.workspaceRoots[projectID] == nil else { return }
        let area = TabArea(projectPath: projectPath)
        state.workspaceRoots[projectID] = .tabArea(area)
        state.focusedAreaID[projectID] = area.id
    }

    private static func resolveArea(projectID: UUID, areaID: UUID?, state: WorkspaceState) -> TabArea? {
        guard let root = state.workspaceRoots[projectID] else { return nil }
        if let areaID {
            return root.findArea(id: areaID)
        }
        guard let focusedID = state.focusedAreaID[projectID] else { return nil }
        return root.findArea(id: focusedID)
    }

    private static func popFocusHistory(projectID: UUID, validAreas: [TabArea], state: inout WorkspaceState) -> UUID? {
        let validIDs = Set(validAreas.map(\.id))
        while let last = state.focusHistory[projectID]?.popLast() {
            if validIDs.contains(last) {
                return last
            }
        }
        return nil
    }
}
