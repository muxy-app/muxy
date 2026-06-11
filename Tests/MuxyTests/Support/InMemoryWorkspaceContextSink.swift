import Foundation

@testable import Muxy

@MainActor
final class InMemoryWorkspaceContextSink: WorkspaceContextSink {
    private(set) var current: WorkspaceContext = .local

    func update(_ context: WorkspaceContext) {
        current = context
    }
}
