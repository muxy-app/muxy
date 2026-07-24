import CoreGraphics
import Foundation

enum DragCoordinateSpace {
    static let mainWindow = "main-window-drag-space"
}

enum TabMoveRequest {
    case toArea(tabID: UUID, sourceAreaID: UUID, destinationAreaID: UUID)
    case toNewSplit(tabID: UUID, sourceAreaID: UUID, targetAreaID: UUID, split: SplitPlacement)
}

struct SplitPlacement {
    let direction: SplitDirection
    let position: SplitPosition
}

enum DropZone: Equatable {
    case left
    case right
    case top
    case bottom
    case center
}

@MainActor
@Observable
final class TabDragCoordinator {
    enum DragSource: Equatable {
        case pane(areaID: UUID)
        case topLevel(groupID: UUID)
    }

    private struct HoverMatch {
        let areaID: UUID
        let frame: CGRect
        let metric: CGFloat
    }

    struct DragInfo: Equatable {
        let tabID: UUID
        let source: DragSource
        let projectID: UUID

        var isTopLevel: Bool {
            if case .topLevel = source {
                return true
            }
            return false
        }
    }

    var activeDrag: DragInfo?
    @ObservationIgnored var globalPosition: CGPoint = .zero
    @ObservationIgnored var areaFramesByProject: [UUID: [UUID: CGRect]] = [:]
    @ObservationIgnored var groupFramesByProject: [UUID: [UUID: CGRect]] = [:]
    private(set) var hoveredAreaID: UUID?
    private(set) var hoveredGroupID: UUID?
    private(set) var hoveredZone: DropZone?

    func setAreaFrames(_ frames: [UUID: CGRect], forProject projectID: UUID) {
        guard areaFramesByProject[projectID] != frames else { return }
        areaFramesByProject[projectID] = frames
        computeHover()
    }

    func setGroupFrames(_ frames: [UUID: CGRect], forProject projectID: UUID) {
        guard groupFramesByProject[projectID] != frames else { return }
        groupFramesByProject[projectID] = frames
        computeHover()
    }

    func beginDrag(tabID: UUID, sourceAreaID: UUID, projectID: UUID) {
        cancelDrag()
        areaFramesByProject.removeValue(forKey: projectID)
        activeDrag = DragInfo(tabID: tabID, source: .pane(areaID: sourceAreaID), projectID: projectID)
    }

    func beginTopLevelDrag(tabID: UUID, sourceGroupID: UUID, projectID: UUID) {
        cancelDrag()
        groupFramesByProject.removeValue(forKey: projectID)
        activeDrag = DragInfo(tabID: tabID, source: .topLevel(groupID: sourceGroupID), projectID: projectID)
    }

    func updatePosition(_ position: CGPoint) {
        globalPosition = position
        computeHover()
    }

    struct DropResult {
        let drag: DragInfo
        let zone: DropZone
        let targetID: UUID

        var targetAreaID: UUID? {
            guard case .pane = drag.source else { return nil }
            return targetID
        }

        var targetGroupID: UUID? {
            guard case .topLevel = drag.source else { return nil }
            return targetID
        }

        func action(projectID: UUID) -> AppState.Action {
            switch drag.source {
            case let .pane(sourceAreaID):
                let request: TabMoveRequest = switch zone {
                case .center:
                    .toArea(tabID: drag.tabID, sourceAreaID: sourceAreaID, destinationAreaID: targetID)
                case .left:
                    .toNewSplit(
                        tabID: drag.tabID, sourceAreaID: sourceAreaID, targetAreaID: targetID,
                        split: SplitPlacement(direction: .horizontal, position: .first)
                    )
                case .right:
                    .toNewSplit(
                        tabID: drag.tabID, sourceAreaID: sourceAreaID, targetAreaID: targetID,
                        split: SplitPlacement(direction: .horizontal, position: .second)
                    )
                case .top:
                    .toNewSplit(
                        tabID: drag.tabID, sourceAreaID: sourceAreaID, targetAreaID: targetID,
                        split: SplitPlacement(direction: .vertical, position: .first)
                    )
                case .bottom:
                    .toNewSplit(
                        tabID: drag.tabID, sourceAreaID: sourceAreaID, targetAreaID: targetID,
                        split: SplitPlacement(direction: .vertical, position: .second)
                    )
                }
                return .moveTab(projectID: projectID, request: request)
            case let .topLevel(sourceGroupID):
                let request: TopLevelTabMoveRequest = switch zone {
                case .center:
                    .toGroup(
                        tabID: drag.tabID,
                        sourceGroupID: sourceGroupID,
                        destinationGroupID: targetID
                    )
                case .left:
                    .toNewSplit(
                        tabID: drag.tabID, sourceGroupID: sourceGroupID, targetGroupID: targetID,
                        split: SplitPlacement(direction: .horizontal, position: .first)
                    )
                case .right:
                    .toNewSplit(
                        tabID: drag.tabID, sourceGroupID: sourceGroupID, targetGroupID: targetID,
                        split: SplitPlacement(direction: .horizontal, position: .second)
                    )
                case .top:
                    .toNewSplit(
                        tabID: drag.tabID, sourceGroupID: sourceGroupID, targetGroupID: targetID,
                        split: SplitPlacement(direction: .vertical, position: .first)
                    )
                case .bottom:
                    .toNewSplit(
                        tabID: drag.tabID, sourceGroupID: sourceGroupID, targetGroupID: targetID,
                        split: SplitPlacement(direction: .vertical, position: .second)
                    )
                }
                return .moveTopLevelTab(projectID: projectID, request: request)
            }
        }
    }

    func endDrag() -> DropResult? {
        guard let activeDrag, let hoveredZone else {
            cancelDrag()
            return nil
        }
        let targetID: UUID? = switch activeDrag.source {
        case .pane:
            hoveredAreaID
        case .topLevel:
            hoveredGroupID
        }
        guard let targetID else {
            cancelDrag()
            return nil
        }
        let result = DropResult(drag: activeDrag, zone: hoveredZone, targetID: targetID)
        cancelDrag()
        return result
    }

    func cancelDrag() {
        if let activeDrag {
            switch activeDrag.source {
            case .pane:
                areaFramesByProject.removeValue(forKey: activeDrag.projectID)
            case .topLevel:
                groupFramesByProject.removeValue(forKey: activeDrag.projectID)
            }
        }
        activeDrag = nil
        globalPosition = .zero
        hoveredAreaID = nil
        hoveredGroupID = nil
        hoveredZone = nil
    }

    private func computeHover() {
        var nextHoveredAreaID: UUID?
        var nextHoveredZone: DropZone?

        guard let activeDrag,
              let frames = frames(for: activeDrag)
        else {
            updateHover(targetID: nil, zone: nil)
            return
        }

        var containingMatch: HoverMatch?
        for (areaID, frame) in frames {
            guard frame.contains(globalPosition) else { continue }
            let dx = globalPosition.x - frame.midX
            let dy = globalPosition.y - frame.midY
            let distanceToCenter = dx * dx + dy * dy

            if let current = containingMatch, current.metric <= distanceToCenter {
                continue
            }
            containingMatch = HoverMatch(areaID: areaID, frame: frame, metric: distanceToCenter)
        }

        if let containingMatch {
            nextHoveredAreaID = containingMatch.areaID
            nextHoveredZone = zone(for: globalPosition, in: containingMatch.frame)
            updateHover(targetID: nextHoveredAreaID, zone: nextHoveredZone)
            return
        }

        let snapTolerance: CGFloat = 8
        var nearestMatch: HoverMatch?

        for (areaID, frame) in frames {
            let distance = distance(from: globalPosition, to: frame)
            guard distance <= snapTolerance else { continue }

            if let current = nearestMatch, current.metric <= distance {
                continue
            }
            nearestMatch = HoverMatch(areaID: areaID, frame: frame, metric: distance)
        }

        guard let nearestMatch else {
            updateHover(targetID: nil, zone: nil)
            return
        }
        let clampedPosition = clamped(globalPosition, to: nearestMatch.frame)
        nextHoveredAreaID = nearestMatch.areaID
        nextHoveredZone = zone(for: clampedPosition, in: nearestMatch.frame)
        updateHover(targetID: nextHoveredAreaID, zone: nextHoveredZone)
    }

    private func frames(for drag: DragInfo) -> [UUID: CGRect]? {
        switch drag.source {
        case .pane:
            areaFramesByProject[drag.projectID]
        case .topLevel:
            groupFramesByProject[drag.projectID]
        }
    }

    private func updateHover(targetID: UUID?, zone: DropZone?) {
        switch activeDrag?.source {
        case .pane:
            if hoveredAreaID != targetID {
                hoveredAreaID = targetID
            }
            if hoveredGroupID != nil {
                hoveredGroupID = nil
            }
        case .topLevel:
            if hoveredGroupID != targetID {
                hoveredGroupID = targetID
            }
            if hoveredAreaID != nil {
                hoveredAreaID = nil
            }
        case nil:
            hoveredAreaID = nil
            hoveredGroupID = nil
        }
        if hoveredZone != zone {
            hoveredZone = zone
        }
    }

    private func distance(from point: CGPoint, to rect: CGRect) -> CGFloat {
        let dx = max(rect.minX - point.x, max(0, point.x - rect.maxX))
        let dy = max(rect.minY - point.y, max(0, point.y - rect.maxY))
        return hypot(dx, dy)
    }

    private func clamped(_ point: CGPoint, to rect: CGRect) -> CGPoint {
        CGPoint(
            x: min(max(point.x, rect.minX), rect.maxX),
            y: min(max(point.y, rect.minY), rect.maxY)
        )
    }

    private func zone(for point: CGPoint, in rect: CGRect) -> DropZone {
        guard rect.width > 0, rect.height > 0 else {
            return .center
        }
        let relX = (point.x - rect.minX) / rect.width
        let relY = (point.y - rect.minY) / rect.height

        let edgeThreshold: CGFloat = 0.3

        if relX < edgeThreshold {
            return .left
        }
        if relX > 1 - edgeThreshold {
            return .right
        }
        if relY < edgeThreshold {
            return .top
        }
        if relY > 1 - edgeThreshold {
            return .bottom
        }
        return .center
    }
}
