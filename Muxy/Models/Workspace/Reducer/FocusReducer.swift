import Foundation

@MainActor
enum FocusReducer {
    enum Direction {
        case left
        case right
        case up
        case down
    }

    private static let focusHistoryLimit = 20

    static func focusArea(_ areaID: UUID, key: WorktreeKey, state: inout WorkspaceState) {
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

    static func focusArea(projectID: UUID, areaID: UUID, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return }
        focusArea(areaID, key: key, state: &state)
    }

    static func focusPane(projectID: UUID, direction: Direction, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state) else { return }
        focusPane(key: key, direction: direction, state: &state)
    }

    static func cycleTabAcrossPanes(projectID: UUID, forward: Bool, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let root = state.workspaceRoots[key],
              let focusedID = state.focusedAreaID[key],
              let activeTab = root.findArea(id: focusedID)?.activeTab,
              let visibleLayout = root.visibleLayout(forTopLevelTabID: activeTab.parentTabID ?? activeTab.id)
        else { return }
        let frames = visibleLayout.areaFrames()
        let sortedPanes = visibleLayout.allPanes().sorted { lhs, rhs in
            guard let lhsFrame = frames[lhs.area.id], let rhsFrame = frames[rhs.area.id] else { return false }
            if lhsFrame.minY != rhsFrame.minY {
                return lhsFrame.minY < rhsFrame.minY
            }
            return lhsFrame.minX < rhsFrame.minX
        }
        guard sortedPanes.count > 1,
              let currentIndex = sortedPanes.firstIndex(where: { $0.area.id == focusedID })
        else { return }
        let nextIndex = forward
            ? (currentIndex + 1) % sortedPanes.count
            : (currentIndex - 1 + sortedPanes.count) % sortedPanes.count
        let next = sortedPanes[nextIndex]
        TabReducer.selectTab(projectID: projectID, areaID: next.area.id, tabID: next.tab.id, state: &state)
    }

    static func popFocusHistory(key: WorktreeKey, validAreas: [TabArea], state: inout WorkspaceState) -> UUID? {
        let validIDs = Set(validAreas.map(\.id))
        while let last = state.focusHistory[key]?.popLast() {
            if validIDs.contains(last) {
                return last
            }
        }
        return nil
    }

    private static func focusPane(key: WorktreeKey, direction: Direction, state: inout WorkspaceState) {
        guard let root = state.workspaceRoots[key],
              let focusedID = state.focusedAreaID[key],
              let activeTab = root.findArea(id: focusedID)?.activeTab,
              let visibleLayout = root.visibleLayout(forTopLevelTabID: activeTab.parentTabID ?? activeTab.id)
        else { return }

        let frames = visibleLayout.areaFrames()
        guard let focusedFrame = frames[focusedID] else { return }

        let bestCandidate = nearestCandidate(
            from: focusedID,
            focusedFrame: focusedFrame,
            frames: frames,
            direction: direction
        )
        guard let bestCandidate,
              let target = visibleLayout.allPanes().first(where: { $0.area.id == bestCandidate })
        else { return }
        TabReducer.selectTab(key: key, areaID: target.area.id, tabID: target.tab.id, state: &state)
    }

    static func movePane(projectID: UUID, direction: Direction, state: inout WorkspaceState) {
        guard let key = WorkspaceReducerShared.activeKey(projectID: projectID, state: state),
              let root = state.workspaceRoots[key],
              let focusedID = state.focusedAreaID[key],
              let activeTab = root.findArea(id: focusedID)?.activeTab,
              let visibleLayout = root.visibleLayout(forTopLevelTabID: activeTab.parentTabID ?? activeTab.id)
        else { return }
        let frames = visibleLayout.areaFrames()
        guard let focusedFrame = frames[focusedID],
              let targetID = nearestCandidate(
                  from: focusedID,
                  focusedFrame: focusedFrame,
                  frames: frames,
                  direction: direction
              ),
              let sourcePane = visibleLayout.allPanes().first(where: { $0.area.id == focusedID }),
              let targetPane = visibleLayout.allPanes().first(where: { $0.area.id == targetID }),
              let sourceIndex = sourcePane.area.tabs.firstIndex(where: { $0.id == sourcePane.tab.id }),
              let targetIndex = targetPane.area.tabs.firstIndex(where: { $0.id == targetPane.tab.id })
        else { return }
        sourcePane.area.tabs[sourceIndex] = targetPane.tab
        targetPane.area.tabs[targetIndex] = sourcePane.tab
        if sourcePane.area.activeTabID == sourcePane.tab.id {
            sourcePane.area.activeTabID = targetPane.tab.id
        }
        if targetPane.area.activeTabID == targetPane.tab.id {
            targetPane.area.activeTabID = sourcePane.tab.id
        }
        focusArea(targetID, key: key, state: &state)
    }

    private static func nearestCandidate(
        from focusedID: UUID,
        focusedFrame: CGRect,
        frames: [UUID: CGRect],
        direction: Direction
    ) -> UUID? {
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
        return bestCandidate
    }

    private struct PaneFocusScore: Comparable {
        let overlapPenalty: Int
        let axisGap: CGFloat
        let crossDistance: CGFloat
        let centerDistance: CGFloat

        static func < (lhs: PaneFocusScore, rhs: PaneFocusScore) -> Bool {
            if lhs.overlapPenalty != rhs.overlapPenalty {
                return lhs.overlapPenalty < rhs.overlapPenalty
            }
            if lhs.axisGap != rhs.axisGap {
                return lhs.axisGap < rhs.axisGap
            }
            if lhs.crossDistance != rhs.crossDistance {
                return lhs.crossDistance < rhs.crossDistance
            }
            return lhs.centerDistance < rhs.centerDistance
        }
    }

    private static func isCandidate(_ candidate: CGRect, from focused: CGRect, direction: Direction) -> Bool {
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
        direction: Direction
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
}
