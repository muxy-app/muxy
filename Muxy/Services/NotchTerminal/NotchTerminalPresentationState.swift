import Foundation

enum NotchTerminalPresentationPhase: Equatable {
    case hidden
    case showing
    case visible
    case hiding
}

struct NotchTerminalPresentationTransition: Equatable {
    let identifier: UInt
    let showsPanel: Bool
}

struct NotchTerminalPresentationState {
    private(set) var phase: NotchTerminalPresentationPhase = .hidden
    private(set) var targetIsVisible = false
    private var transitionIdentifier: UInt = 0

    mutating func requestVisibility(_ isVisible: Bool) -> NotchTerminalPresentationTransition? {
        guard targetIsVisible != isVisible else { return nil }
        targetIsVisible = isVisible
        transitionIdentifier &+= 1
        phase = isVisible ? .showing : .hiding
        return NotchTerminalPresentationTransition(
            identifier: transitionIdentifier,
            showsPanel: isVisible
        )
    }

    mutating func complete(_ transition: NotchTerminalPresentationTransition) -> Bool {
        guard transition.identifier == transitionIdentifier,
              transition.showsPanel == targetIsVisible
        else { return false }
        phase = targetIsVisible ? .visible : .hidden
        return true
    }
}
