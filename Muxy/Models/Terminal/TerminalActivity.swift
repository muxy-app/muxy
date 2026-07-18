import Foundation

enum TerminalActivity: Equatable {
    case working(TerminalProgress)
    case waiting
    case unread(Int)
    case finished

    var isUnread: Bool {
        if case .unread = self {
            return true
        }
        return false
    }

    static func resolve(
        progress: TerminalProgress?,
        agentStatus: AgentStatus?,
        unreadCount: Int,
        completionPending: Bool
    ) -> TerminalActivity? {
        if let progress {
            return .working(progress)
        }
        if agentStatus == .working {
            return .working(TerminalProgress(kind: .indeterminate, percent: nil))
        }
        if agentStatus == .waiting {
            return .waiting
        }
        if unreadCount > 0 {
            return .unread(unreadCount)
        }
        if completionPending {
            return .finished
        }
        return nil
    }
}
