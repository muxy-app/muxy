import Foundation

@testable import Muxy

extension TerminalViewRemoving {
    func releaseViewPreservingSession(for _: UUID) {}

    func hasPersistentSession(for _: UUID, sessionID _: UUID) -> Bool {
        false
    }
}
