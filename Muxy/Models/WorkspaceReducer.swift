import Foundation

@MainActor
struct WorkspaceState {
    var activeProjectID: UUID?
    var activeWorktreeID: [UUID: UUID]
    var workspaceRoots: [WorktreeKey: SplitNode]
    var focusedAreaID: [WorktreeKey: UUID]
    var focusHistory: [WorktreeKey: [UUID]]
}

@MainActor
struct WorkspaceSideEffects {
    var paneIDsToRemove: [UUID] = []
    var projectIDsToRemove: [UUID] = []
}

@MainActor
enum WorkspaceReducer {
    private struct WorktreeReplacement {
        let id: UUID
        let path: String
    }

    static func reduce(action: AppState.Action, state: inout WorkspaceState) -> WorkspaceSideEffects {
        var effects = WorkspaceSideEffects()

        switch action {
        case let .selectProject(projectID, worktreeID, worktreePath):
            state.activeProjectID = projectID
            state.activeWorktreeID[projectID] = worktreeID
            ensureWorkspaceExists(
                projectID: projectID,
                worktreeID: worktreeID,
                worktreePath: worktreePath,
                state: &state
            )

        case let .selectWorktree(projectID, worktreeID, worktreePath):
            state.activeProjectID = projectID
            state.activeWorktreeID[projectID] = worktreeID
            ensureWorkspaceExists(
                projectID: projectID,
                worktreeID: worktreeID,
                worktreePath: worktreePath,
                state: &state
            )

        case let .removeProject(projectID):
            removeProject(projectID: projectID, state: &state, effects: &effects)

        case let .removeWorktree(projectID, worktreeID, replacementWorktreeID, replacementWorktreePath):
            let replacement: WorktreeReplacement? = if let replacementWorktreeID,
                                                       let replacementWorktreePath
            {
                WorktreeReplacement(id: replacementWorktreeID, path: replacementWorktreePath)
            } else {
                nil
            }
            removeWorktree(
                projectID: projectID,
                worktreeID: worktreeID,
                replacement: replacement,
                state: &state,
                effects: &effects
            )

        case let .createTab(projectID, areaID):
            guard let key = activeKey(projectID: projectID, state: state),
                  let area = resolveArea(key: key, areaID: areaID, state: state)
            else { break }
            focusArea(area.id, key: key, state: &state)
            area.createTab()

        case let .createVCSTab(projectID, areaID):
            guard let key = activeKey(projectID: projectID, state: state),
                  let area = resolveArea(key: key, areaID: areaID, state: state)
            else { break }
            focusArea(area.id, key: key, state: &state)
            area.createVCSTab()

        case let .createEditorTab(projectID, areaID, filePath):
            guard let key = activeKey(projectID: projectID, state: state),
                  let area = resolveArea(key: key, areaID: areaID, state: state)
            else { break }
            focusArea(area.id, key: key, state: &state)
            area.createEditorTab(filePath: filePath)

        case let .closeTab(projectID, areaID, tabID):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            closeTab(tabID, areaID: areaID, key: key, state: &state, effects: &effects)

        case let .selectTab(projectID, areaID, tabID):
            guard let key = activeKey(projectID: projectID, state: state),
                  let area = resolveArea(key: key, areaID: areaID, state: state)
            else { break }
            focusArea(area.id, key: key, state: &state)
            area.selectTab(tabID)

        case let .selectTabByIndex(projectID, areaID, index):
            guard let key = activeKey(projectID: projectID, state: state),
                  let area = resolveArea(key: key, areaID: areaID, state: state)
            else { break }
            focusArea(area.id, key: key, state: &state)
            area.selectTabByIndex(index)

        case let .selectNextTab(projectID):
            guard let key = activeKey(projectID: projectID, state: state),
                  let area = resolveArea(key: key, areaID: nil, state: state)
            else { break }
            area.selectNextTab()

        case let .selectPreviousTab(projectID):
            guard let key = activeKey(projectID: projectID, state: state),
                  let area = resolveArea(key: key, areaID: nil, state: state)
            else { break }
            area.selectPreviousTab()

        case let .splitArea(request):
            splitArea(request, state: &state)

        case let .closeArea(projectID, areaID):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            closeArea(areaID, key: key, state: &state, effects: &effects)

        case let .moveTab(projectID, request):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            moveTab(request, key: key, state: &state, effects: &effects)

        case let .focusArea(projectID, areaID):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            focusArea(areaID, key: key, state: &state)

        case let .focusPaneLeft(projectID):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            focusPane(key: key, direction: .left, state: &state)

        case let .focusPaneRight(projectID):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            focusPane(key: key, direction: .right, state: &state)

        case let .focusPaneUp(projectID):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            focusPane(key: key, direction: .up, state: &state)

        case let .focusPaneDown(projectID):
            guard let key = activeKey(projectID: projectID, state: state) else { break }
            focusPane(key: key, direction: .down, state: &state)

        case let .selectNextProject(projects, worktrees):
            cycleProject(projects: projects, worktrees: worktrees, forward: true, state: &state)

        case let .selectPreviousProject(projects, worktrees):
            cycleProject(projects: projects, worktrees: worktrees, forward: false, state: &state)
        }

        return effects
    }

    private static func activeKey(projectID: UUID, state: WorkspaceState) -> WorktreeKey? {
        guard let worktreeID = state.activeWorktreeID[projectID] else { return nil }
        return WorktreeKey(projectID: projectID, worktreeID: worktreeID)
    }

    private static func splitArea(_ request: AppState.SplitAreaRequest, state: inout WorkspaceState) {
        guard let key = activeKey(projectID: request.projectID, state: state),
              let root = state.workspaceRoots[key]
        else { return }
        let (newRoot, newAreaID) = root.splitting(
            areaID: request.areaID,
            direction: request.direction,
            position: request.position
        )
        state.workspaceRoots[key] = newRoot
        guard let newAreaID else { return }
        focusArea(newAreaID, key: key, state: &state)
    }

    private static func closeArea(
        _ areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        let removed = removeAreaFromTree(areaID, key: key, state: &state, effects: &effects)
        guard !removed else { return }
        clearWorkspace(key: key, state: &state)
        handleProjectEmptiedIfNeeded(projectID: key.projectID, state: &state, effects: &effects)
    }

    private static func clearWorkspace(key: WorktreeKey, state: inout WorkspaceState) {
        state.workspaceRoots.removeValue(forKey: key)
        state.focusedAreaID.removeValue(forKey: key)
        state.focusHistory.removeValue(forKey: key)
    }

    private static func handleProjectEmptiedIfNeeded(
        projectID: UUID,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        let hasAnyWorkspace = state.workspaceRoots.keys.contains { $0.projectID == projectID }
        guard !hasAnyWorkspace else { return }
        state.activeWorktreeID.removeValue(forKey: projectID)
        if state.activeProjectID == projectID {
            state.activeProjectID = nil
        }
        effects.projectIDsToRemove.append(projectID)
    }

    private static func moveTab(
        _ request: TabMoveRequest,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        switch request {
        case let .toArea(tabID, sourceAreaID, destinationAreaID):
            guard sourceAreaID != destinationAreaID else { return }
            guard let root = state.workspaceRoots[key],
                  let sourceArea = root.findArea(id: sourceAreaID),
                  let destArea = root.findArea(id: destinationAreaID),
                  let tab = sourceArea.removeTab(tabID)
            else { return }

            destArea.insertExistingTab(tab)
            focusArea(destinationAreaID, key: key, state: &state)

            guard sourceArea.tabs.isEmpty else { return }
            collapseEmptyArea(sourceAreaID, key: key, state: &state, effects: &effects)

        case let .toNewSplit(tabID, sourceAreaID, targetAreaID, split):
            guard let root = state.workspaceRoots[key],
                  let sourceArea = root.findArea(id: sourceAreaID),
                  let tab = sourceArea.removeTab(tabID)
            else { return }

            let shouldCollapseSource = sourceArea.tabs.isEmpty
            if shouldCollapseSource, sourceAreaID != targetAreaID {
                collapseEmptyArea(sourceAreaID, key: key, state: &state, effects: &effects)
            }

            guard let currentRoot = state.workspaceRoots[key] else { return }
            let (newRoot, newAreaID) = currentRoot.splittingWithTab(
                areaID: targetAreaID,
                direction: split.direction,
                position: split.position,
                tab: tab
            )
            state.workspaceRoots[key] = newRoot

            if let newAreaID {
                focusArea(newAreaID, key: key, state: &state)
            }

            guard shouldCollapseSource, sourceAreaID == targetAreaID else { return }
            if let updatedRoot = state.workspaceRoots[key],
               let emptyArea = updatedRoot.findArea(id: targetAreaID),
               emptyArea.tabs.isEmpty
            {
                collapseEmptyArea(targetAreaID, key: key, state: &state, effects: &effects)
            }
        }
    }

    private static func collapseEmptyArea(
        _ areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        _ = removeAreaFromTree(areaID, key: key, state: &state, effects: &effects)
    }

    @discardableResult
    private static func removeAreaFromTree(
        _ areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) -> Bool {
        guard let root = state.workspaceRoots[key] else { return false }
        if let area = root.findArea(id: areaID) {
            effects.paneIDsToRemove.append(contentsOf: area.tabs.compactMap { $0.content.pane?.id })
        }
        guard let newRoot = root.removing(areaID: areaID) else { return false }
        state.workspaceRoots[key] = newRoot
        state.focusHistory[key]?.removeAll { $0 == areaID }
        guard state.focusedAreaID[key] == areaID else { return true }
        let remaining = newRoot.allAreas()
        let previousID = popFocusHistory(key: key, validAreas: remaining, state: &state)
        state.focusedAreaID[key] = previousID ?? remaining.first?.id
        return true
    }

    private static func closeTab(
        _ tabID: UUID,
        areaID: UUID,
        key: WorktreeKey,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        guard let root = state.workspaceRoots[key],
              let area = root.findArea(id: areaID)
        else { return }

        let areaCount = root.allAreas().count
        if area.tabs.count <= 1, areaCount > 1 {
            closeArea(areaID, key: key, state: &state, effects: &effects)
            return
        }

        if let paneID = area.closeTab(tabID) {
            effects.paneIDsToRemove.append(paneID)
        }

        guard area.tabs.isEmpty else { return }
        clearWorkspace(key: key, state: &state)
        handleProjectEmptiedIfNeeded(projectID: key.projectID, state: &state, effects: &effects)
    }

    private static let focusHistoryLimit = 20

    private static func focusArea(_ areaID: UUID, key: WorktreeKey, state: inout WorkspaceState) {
        if let current = state.focusedAreaID[key], current != areaID {
            var history = state.focusHistory[key, default: []]
            history.append(current)
            if history.count > focusHistoryLimit {
                history.removeFirst(history.count - focusHistoryLimit)
            }
            state.focusHistory[key] = history
        }
        state.focusedAreaID[key] = areaID
    }

    private static func cycleProject(
        projects: [Project],
        worktrees: [UUID: [Worktree]],
        forward: Bool,
        state: inout WorkspaceState
    ) {
        guard projects.count > 1,
              let currentID = state.activeProjectID,
              let index = projects.firstIndex(where: { $0.id == currentID })
        else { return }
        let next = forward ? (index + 1) % projects.count : (index - 1 + projects.count) % projects.count
        let project = projects[next]
        let list = worktrees[project.id] ?? []
        let existingID = state.activeWorktreeID[project.id]
        let target = list.first(where: { $0.id == existingID })
            ?? list.first(where: { $0.isPrimary })
            ?? list.first
        guard let worktree = target else { return }
        state.activeProjectID = project.id
        state.activeWorktreeID[project.id] = worktree.id
        ensureWorkspaceExists(
            projectID: project.id,
            worktreeID: worktree.id,
            worktreePath: worktree.path,
            state: &state
        )
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

    private static func focusPane(key: WorktreeKey, direction: PaneFocusDirection, state: inout WorkspaceState) {
        guard let root = state.workspaceRoots[key],
              let focusedID = state.focusedAreaID[key]
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
        focusArea(bestCandidate, key: key, state: &state)
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
        let keysToRemove = state.workspaceRoots.keys.filter { $0.projectID == projectID }
        for key in keysToRemove {
            if let root = state.workspaceRoots[key] {
                let paneIDs = root.allAreas().flatMap { area in area.tabs.compactMap { $0.content.pane?.id } }
                effects.paneIDsToRemove.append(contentsOf: paneIDs)
            }
            state.workspaceRoots.removeValue(forKey: key)
            state.focusedAreaID.removeValue(forKey: key)
            state.focusHistory.removeValue(forKey: key)
        }
        state.activeWorktreeID.removeValue(forKey: projectID)
        if state.activeProjectID == projectID {
            state.activeProjectID = nil
        }
    }

    private static func removeWorktree(
        projectID: UUID,
        worktreeID: UUID,
        replacement: WorktreeReplacement?,
        state: inout WorkspaceState,
        effects: inout WorkspaceSideEffects
    ) {
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        if let root = state.workspaceRoots[key] {
            let paneIDs = root.allAreas().flatMap { area in area.tabs.compactMap { $0.content.pane?.id } }
            effects.paneIDsToRemove.append(contentsOf: paneIDs)
        }
        state.workspaceRoots.removeValue(forKey: key)
        state.focusedAreaID.removeValue(forKey: key)
        state.focusHistory.removeValue(forKey: key)

        guard state.activeWorktreeID[projectID] == worktreeID else { return }
        if let replacement {
            state.activeWorktreeID[projectID] = replacement.id
            ensureWorkspaceExists(
                projectID: projectID,
                worktreeID: replacement.id,
                worktreePath: replacement.path,
                state: &state
            )
            return
        }

        let hasProjectWorkspace = state.workspaceRoots.keys.contains { $0.projectID == projectID }
        if hasProjectWorkspace,
           let fallback = state.workspaceRoots.keys
           .filter({ $0.projectID == projectID })
           .min(by: { $0.worktreeID.uuidString < $1.worktreeID.uuidString })
        {
            state.activeWorktreeID[projectID] = fallback.worktreeID
            return
        }

        state.activeWorktreeID.removeValue(forKey: projectID)
        if state.activeProjectID == projectID {
            state.activeProjectID = nil
        }
    }

    private static func ensureWorkspaceExists(
        projectID: UUID,
        worktreeID: UUID,
        worktreePath: String,
        state: inout WorkspaceState
    ) {
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        guard state.workspaceRoots[key] == nil else { return }
        let area = TabArea(projectPath: worktreePath)
        state.workspaceRoots[key] = .tabArea(area)
        state.focusedAreaID[key] = area.id
    }

    private static func resolveArea(key: WorktreeKey, areaID: UUID?, state: WorkspaceState) -> TabArea? {
        guard let root = state.workspaceRoots[key] else { return nil }
        if let areaID {
            return root.findArea(id: areaID)
        }
        guard let focusedID = state.focusedAreaID[key] else { return nil }
        return root.findArea(id: focusedID)
    }

    private static func popFocusHistory(key: WorktreeKey, validAreas: [TabArea], state: inout WorkspaceState) -> UUID? {
        let validIDs = Set(validAreas.map(\.id))
        while let last = state.focusHistory[key]?.popLast() {
            if validIDs.contains(last) {
                return last
            }
        }
        return nil
    }
}
