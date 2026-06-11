import Foundation

@MainActor
protocol WorkspaceContextSink: AnyObject {
    func update(_ context: WorkspaceContext)
}

@MainActor
@Observable
final class ActiveWorkspaceContext: WorkspaceContextSink {
    static let shared = ActiveWorkspaceContext()

    private(set) var current: WorkspaceContext = .local

    func update(_ context: WorkspaceContext) {
        guard context != current else { return }
        current = context
    }
}
