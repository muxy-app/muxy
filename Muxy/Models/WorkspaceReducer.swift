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
    var projectIDsToRemove: [UUID] = []
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

        case let .createVCSTab(projectID, areaID):
            guard let area = resolveArea(projectID: projectID, areaID: areaID, state: state) else { break }
            focusArea(area.id, projectID: projectID, state: &state)
            area.createVCSTab()

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

        case let .splitArea(request):
            splitArea(request, state: &state)

        case let .closeArea(projectID, areaID):
            closeArea(areaID, projectID: projectID, state: &state, effects: &effects)

        case let .moveTab(projectID, request):
            moveTab(request, projectID: projectID, state: &state, effects: &effects)

        case let .focusArea(projectID, areaID):
            focusArea(areaID, projectID: projectID, state: &state)

        case let .focusPaneLeft(projectID):
            focusPane(projectID: projectID, direction: .left, state: &state)

        case let .focusPaneRight(projectID):
            focusPane(projectID: projectID, direction: .right, state: &state)

        case let .focusPaneUp(projectID):
            focusPane(projectID: projectID, direction: .up, state: &state)

        case let .focusPaneDown(projectID):
            focusPane(projectID: projectID, direction: .down, state: &state)

        case let .selectNextProject(projects):
            cycleProject(projects: projects, forward: true, state: &state)

        case let .selectPreviousProject(projects):
            cycleProject(projects: projects, forward: false, state: &state)
        }

        return effects
    }

    private static func splitArea(_ request: AppState.SplitAreaRequest, state: inout WorkspaceState) {
        guard let root = state.workspaceRoots[request.projectID] else { return }
        let (newRoot, newAreaID) = root.splitting(
            areaID: request.areaID,
            direction: request.direction,
            position: request.position,
            projectPath: request.projectPath
        )
        state.workspaceRoots[request.projectID] = newRoot
        guard let newAreaID else { return }
        if let current = state.focusedAreaID[request.projectID] {
            state.focusHistory[request.projectID, default: []].append(current)
        }
        state.focusedAreaID[request.projectID] = newAreaID
    }

    private static func closeArea(
        _ areaID: UUID,
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        let removed = removeAreaFromTree(areaID, projectID: projectID, state: &state, effects: &effects)
        guard !removed else { return }
        state.workspaceRoots.removeValue(forKey: projectID)
        state.focusedAreaID.removeValue(forKey: projectID)
        state.focusHistory.removeValue(forKey: projectID)
        state.activeProjectID = nil
        effects.projectIDsToRemove.append(projectID)
    }

    private static func moveTab(
        _ request: TabMoveRequest,
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        switch request {
        case let .toArea(tabID, sourceAreaID, destinationAreaID):
            guard sourceAreaID != destinationAreaID else { return }
            guard let root = state.workspaceRoots[projectID],
                  let sourceArea = root.findArea(id: sourceAreaID),
                  let destArea = root.findArea(id: destinationAreaID),
                  let tab = sourceArea.removeTab(tabID)
            else { return }

            destArea.insertExistingTab(tab)
            focusArea(destinationAreaID, projectID: projectID, state: &state)

            guard sourceArea.tabs.isEmpty else { return }
            collapseEmptyArea(sourceAreaID, projectID: projectID, state: &state, effects: &effects)

        case let .toNewSplit(tabID, sourceAreaID, targetAreaID, split):
            guard let root = state.workspaceRoots[projectID],
                  let sourceArea = root.findArea(id: sourceAreaID),
                  let tab = sourceArea.removeTab(tabID)
            else { return }

            let shouldCollapseSource = sourceArea.tabs.isEmpty
            if shouldCollapseSource, sourceAreaID != targetAreaID {
                collapseEmptyArea(sourceAreaID, projectID: projectID, state: &state, effects: &effects)
            }

            guard let currentRoot = state.workspaceRoots[projectID] else { return }
            let (newRoot, newAreaID) = currentRoot.splittingWithTab(
                areaID: targetAreaID,
                direction: split.direction,
                position: split.position,
                tab: tab,
                projectPath: sourceArea.projectPath
            )
            state.workspaceRoots[projectID] = newRoot

            if let newAreaID {
                focusArea(newAreaID, projectID: projectID, state: &state)
            }

            guard shouldCollapseSource, sourceAreaID == targetAreaID else { return }
            if let updatedRoot = state.workspaceRoots[projectID],
               let emptyArea = updatedRoot.findArea(id: targetAreaID),
               emptyArea.tabs.isEmpty
            {
                collapseEmptyArea(targetAreaID, projectID: projectID, state: &state, effects: &effects)
            }
        }
    }

    private static func collapseEmptyArea(
        _ areaID: UUID,
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        _ = removeAreaFromTree(areaID, projectID: projectID, state: &state, effects: &effects)
    }

    @discardableResult
    private static func removeAreaFromTree(
        _ areaID: UUID,
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) -> Bool {
        guard let root = state.workspaceRoots[projectID] else { return false }
        if let area = root.findArea(id: areaID) {
            effects.paneIDsToRemove.append(contentsOf: area.tabs.compactMap { $0.content.pane?.id })
        }
        guard let newRoot = root.removing(areaID: areaID) else { return false }
        state.workspaceRoots[projectID] = newRoot
        state.focusHistory[projectID]?.removeAll { $0 == areaID }
        guard state.focusedAreaID[projectID] == areaID else { return true }
        let remaining = newRoot.allAreas()
        let previousID = popFocusHistory(projectID: projectID, validAreas: remaining, state: &state)
        state.focusedAreaID[projectID] = previousID ?? remaining.first?.id
        return true
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
        effects.projectIDsToRemove.append(projectID)
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

    private struct PaneFocusScore: Comparable {
        let overlapPenalty: Int
        let axisGap: CGFloat
        let crossDistance: CGFloat
        let centerDistance: CGFloat

        static func < (lhs: PaneFocusScore, rhs: PaneFocusScore) -> Bool {
            if lhs.overlapPenalty != rhs.overlapPenalty { return lhs.overlapPenalty < rhs.overlapPenalty }
            if lhs.axisGap != rhs.axisGap { return lhs.axisGap < rhs.axisGap }
            if lhs.crossDistance != rhs.crossDistance { return lhs.crossDistance < rhs.crossDistance }
            return lhs.centerDistance < rhs.centerDistance
        }
    }

    private enum PaneFocusDirection {
        case left
        case right
        case up
        case down
    }

    private static func focusPane(projectID: UUID, direction: PaneFocusDirection, state: inout WorkspaceState) {
        guard let root = state.workspaceRoots[projectID],
              let focusedID = state.focusedAreaID[projectID]
        else { return }

        let frames = root.areaFrames()
        guard let focusedFrame = frames[focusedID] else { return }

        var bestCandidate: UUID?
        var bestScore: PaneFocusScore?

        for (candidateID, candidateFrame) in frames where candidateID != focusedID {
            guard isCandidate(candidateFrame, from: focusedFrame, direction: direction) else { continue }

            let score = scoreForCandidate(candidateFrame, from: focusedFrame, direction: direction)
            if bestScore.map({ score < $0 }) ?? true {
                bestCandidate = candidateID
                bestScore = score
            }
        }

        guard let bestCandidate else { return }
        focusArea(bestCandidate, projectID: projectID, state: &state)
    }

    private static func isCandidate(_ candidate: CGRect, from focused: CGRect, direction: PaneFocusDirection) -> Bool {
        switch direction {
        case .left: candidate.midX < focused.midX
        case .right: candidate.midX > focused.midX
        case .up: candidate.midY < focused.midY
        case .down: candidate.midY > focused.midY
        }
    }

    private static func scoreForCandidate(
        _ candidate: CGRect,
        from focused: CGRect,
        direction: PaneFocusDirection
    ) -> PaneFocusScore {
        let overlap: CGFloat
        let axisGap: CGFloat
        let crossDistance: CGFloat
        let centerDistance: CGFloat

        switch direction {
        case .left:
            overlap = min(focused.maxY, candidate.maxY) - max(focused.minY, candidate.minY)
            axisGap = max(0, focused.minX - candidate.maxX)
            crossDistance = abs(focused.midY - candidate.midY)
            centerDistance = abs(focused.midX - candidate.midX)
        case .right:
            overlap = min(focused.maxY, candidate.maxY) - max(focused.minY, candidate.minY)
            axisGap = max(0, candidate.minX - focused.maxX)
            crossDistance = abs(focused.midY - candidate.midY)
            centerDistance = abs(focused.midX - candidate.midX)
        case .up:
            overlap = min(focused.maxX, candidate.maxX) - max(focused.minX, candidate.minX)
            axisGap = max(0, focused.minY - candidate.maxY)
            crossDistance = abs(focused.midX - candidate.midX)
            centerDistance = abs(focused.midY - candidate.midY)
        case .down:
            overlap = min(focused.maxX, candidate.maxX) - max(focused.minX, candidate.minX)
            axisGap = max(0, candidate.minY - focused.maxY)
            crossDistance = abs(focused.midX - candidate.midX)
            centerDistance = abs(focused.midY - candidate.midY)
        }

        return PaneFocusScore(
            overlapPenalty: overlap > 0 ? 0 : 1,
            axisGap: axisGap,
            crossDistance: crossDistance,
            centerDistance: centerDistance
        )
    }

    private static func removeProject(
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        if let root = state.workspaceRoots[projectID] {
            let paneIDs = root.allAreas().flatMap { area in area.tabs.compactMap { $0.content.pane?.id } }
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
