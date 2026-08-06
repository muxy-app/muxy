import Foundation

enum StableSubsetOrder {
    static func reordering<Element: Hashable>(
        fullOrder: [Element],
        visibleOrder: [Element],
        moving: Element,
        over: Element
    ) -> [Element] {
        guard moving != over,
              let sourceIndex = visibleOrder.firstIndex(of: moving),
              let destinationIndex = visibleOrder.firstIndex(of: over),
              Set(visibleOrder).count == visibleOrder.count,
              Set(visibleOrder).isSubset(of: Set(fullOrder))
        else { return fullOrder }

        var reorderedVisible = visibleOrder
        let moved = reorderedVisible.remove(at: sourceIndex)
        reorderedVisible.insert(moved, at: destinationIndex)

        let visibleIDs = Set(visibleOrder)
        let visibleSlots = fullOrder.indices.filter { visibleIDs.contains(fullOrder[$0]) }
        guard visibleSlots.count == reorderedVisible.count else { return fullOrder }

        var reorderedFull = fullOrder
        for (slot, id) in zip(visibleSlots, reorderedVisible) {
            reorderedFull[slot] = id
        }
        return reorderedFull
    }
}
