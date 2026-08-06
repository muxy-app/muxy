import Foundation
import Testing

@testable import Muxy

@Suite("StableSubsetOrder")
struct StableSubsetOrderTests {
    @Test("reorders visible IDs without moving hidden IDs")
    func reordersVisibleSubset() {
        let reordered = StableSubsetOrder.reordering(
            fullOrder: ["agent-1", "hidden-1", "agent-2", "hidden-2"],
            visibleOrder: ["agent-1", "agent-2"],
            moving: "agent-2",
            over: "agent-1"
        )

        #expect(reordered == ["agent-2", "hidden-1", "agent-1", "hidden-2"])
    }

    @Test("ignores moves outside the visible subset")
    func ignoresUnknownMove() {
        let fullOrder = ["agent-1", "hidden-1", "agent-2"]
        let reordered = StableSubsetOrder.reordering(
            fullOrder: fullOrder,
            visibleOrder: ["agent-1", "agent-2"],
            moving: "agent-1",
            over: "hidden-1"
        )

        #expect(reordered == fullOrder)
    }
}
