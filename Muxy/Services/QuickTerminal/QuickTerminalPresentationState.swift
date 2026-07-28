import Foundation

enum QuickTerminalPresentationPhase: Equatable {
    case hidden
    case showing
    case visible
    case hiding
}

struct QuickTerminalPresentationTransition: Equatable {
    let identifier: UInt
    let showsPanel: Bool
}

struct QuickTerminalPresentationState {
    private(set) var phase: QuickTerminalPresentationPhase = .hidden
    private(set) var targetIsVisible = false
    private var transitionIdentifier: UInt = 0

    mutating func requestVisibility(_ isVisible: Bool) -> QuickTerminalPresentationTransition? {
        guard targetIsVisible != isVisible else { return nil }
        targetIsVisible = isVisible
        transitionIdentifier &+= 1
        phase = isVisible ? .showing : .hiding
        return QuickTerminalPresentationTransition(
            identifier: transitionIdentifier,
            showsPanel: isVisible
        )
    }

    mutating func complete(_ transition: QuickTerminalPresentationTransition) -> Bool {
        guard transition.identifier == transitionIdentifier,
              transition.showsPanel == targetIsVisible
        else { return false }
        phase = targetIsVisible ? .visible : .hidden
        return true
    }
}
