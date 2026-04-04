import Foundation

enum TabMoveRequest {
    case toArea(tabID: UUID, sourceAreaID: UUID, destinationAreaID: UUID)
    case toNewSplit(tabID: UUID, sourceAreaID: UUID, targetAreaID: UUID, split: SplitPlacement)
}

struct SplitPlacement {
    let direction: SplitDirection
    let position: SplitPosition
}
