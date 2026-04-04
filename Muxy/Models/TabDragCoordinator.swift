import CoreGraphics
import Foundation

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
    struct DragInfo: Equatable {
        let tabID: UUID
        let sourceAreaID: UUID
        let projectID: UUID
    }

    var activeDrag: DragInfo?
    var globalPosition: CGPoint = .zero
    var areaFrames: [UUID: CGRect] = [:]
    private(set) var hoveredAreaID: UUID?
    private(set) var hoveredZone: DropZone?

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

        for (areaID, frame) in areaFrames {
            guard frame.contains(globalPosition) else { continue }
            hoveredAreaID = areaID
            hoveredZone = zone(for: globalPosition, in: frame)
            return
        }
    }

    private func zone(for point: CGPoint, in rect: CGRect) -> DropZone {
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
