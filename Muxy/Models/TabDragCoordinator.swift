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
    private struct HoverMatch {
        let areaID: UUID
        let frame: CGRect
        let metric: CGFloat
    }

    struct DragInfo: Equatable {
        let tabID: UUID
        let sourceAreaID: UUID
        let projectID: UUID
    }

    var activeDrag: DragInfo?
    var globalPosition: CGPoint = .zero
    var areaFramesByProject: [UUID: [UUID: CGRect]] = [:]
    private(set) var hoveredAreaID: UUID?
    private(set) var hoveredZone: DropZone?

    func setAreaFrames(_ frames: [UUID: CGRect], forProject projectID: UUID) {
        areaFramesByProject[projectID] = frames
    }

    func beginDrag(tabID: UUID, sourceAreaID: UUID, projectID: UUID) {
        activeDrag = DragInfo(tabID: tabID, sourceAreaID: sourceAreaID, projectID: projectID)
    }

    func updatePosition(_ position: CGPoint) {
        globalPosition = position
        computeHover()
    }

    struct DropResult {
        let drag: DragInfo
        let zone: DropZone
        let targetAreaID: UUID

        func action(projectID: UUID) -> AppState.Action {
            let request: TabMoveRequest = switch zone {
            case .center:
                .toArea(tabID: drag.tabID, sourceAreaID: drag.sourceAreaID, destinationAreaID: targetAreaID)
            case .left:
                .toNewSplit(
                    tabID: drag.tabID, sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                    split: SplitPlacement(direction: .horizontal, position: .first)
                )
            case .right:
                .toNewSplit(
                    tabID: drag.tabID, sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                    split: SplitPlacement(direction: .horizontal, position: .second)
                )
            case .top:
                .toNewSplit(
                    tabID: drag.tabID, sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                    split: SplitPlacement(direction: .vertical, position: .first)
                )
            case .bottom:
                .toNewSplit(
                    tabID: drag.tabID, sourceAreaID: drag.sourceAreaID, targetAreaID: targetAreaID,
                    split: SplitPlacement(direction: .vertical, position: .second)
                )
            }
            return .moveTab(projectID: projectID, request: request)
        }
    }

    func endDrag() -> DropResult? {
        guard let activeDrag, let hoveredAreaID, let hoveredZone else {
            cancelDrag()
            return nil
        }
        let result = DropResult(drag: activeDrag, zone: hoveredZone, targetAreaID: hoveredAreaID)
        cancelDrag()
        return result
    }

    func cancelDrag() {
        activeDrag = nil
        globalPosition = .zero
        hoveredAreaID = nil
        hoveredZone = nil
    }

    private func computeHover() {
        hoveredAreaID = nil
        hoveredZone = nil

        guard let projectID = activeDrag?.projectID,
              let frames = areaFramesByProject[projectID]
        else { return }

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
            hoveredAreaID = containingMatch.areaID
            hoveredZone = zone(for: globalPosition, in: containingMatch.frame)
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

        guard let nearestMatch else { return }
        let clampedPosition = clamped(globalPosition, to: nearestMatch.frame)
        hoveredAreaID = nearestMatch.areaID
        hoveredZone = zone(for: clampedPosition, in: nearestMatch.frame)
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
