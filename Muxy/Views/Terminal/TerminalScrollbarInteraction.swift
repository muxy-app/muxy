import Foundation

struct TerminalScrollbarInteraction {
    static let revealDuration: TimeInterval = 1.25

    private var revealedUntil: Date?
    private(set) var isDragging = false

    mutating func reveal(now: Date) {
        revealedUntil = now.addingTimeInterval(Self.revealDuration)
    }

    mutating func beginDrag() {
        isDragging = true
    }

    mutating func endDrag(now: Date) {
        isDragging = false
        reveal(now: now)
    }

    func allowsScrollerHit(now: Date) -> Bool {
        guard !isDragging else { return true }
        guard let revealedUntil else { return false }
        return now < revealedUntil
    }
}
