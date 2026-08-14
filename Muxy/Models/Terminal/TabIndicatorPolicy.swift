import Foundation

enum TabIndicatorPolicy {
    static func agentStatus(from statuses: [AgentStatus]) -> AgentStatus? {
        if statuses.contains(.waiting) {
            return .waiting
        }
        if statuses.contains(.working) {
            return .working
        }
        return statuses.first
    }

    static func showsAttentionDot(isActive: Bool, hasAttention: Bool, hasUnfocusedAttention: Bool) -> Bool {
        isActive ? hasUnfocusedAttention : hasAttention
    }

    static func newlyPendingPaneIDs(previous: Set<UUID>, current: Set<UUID>) -> Set<UUID> {
        current.subtracting(previous)
    }

    static func completionPaneIDToClear(
        isActive: Bool,
        focusedPaneID: UUID?,
        newlyPendingPaneIDs: Set<UUID>
    ) -> UUID? {
        guard isActive,
              let focusedPaneID,
              newlyPendingPaneIDs.contains(focusedPaneID)
        else { return nil }
        return focusedPaneID
    }
}
