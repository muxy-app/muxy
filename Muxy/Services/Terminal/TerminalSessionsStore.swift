import Foundation
import MuxySessionProtocol

@MainActor
@Observable
final class TerminalSessionsStore {
    static let shared = TerminalSessionsStore()

    private(set) var sessions: [SessionDescriptor] = []

    @ObservationIgnored private var refreshTask: Task<Void, Never>?

    private init() {}

    func refresh() {
        guard TerminalPersistentSessionPreferences.isEnabled else {
            sessions = []
            return
        }
        guard refreshTask == nil else { return }
        refreshTask = Task { [weak self] in
            let latest = await PersistentSessionService.shared.liveSessions()
            guard let self else { return }
            refreshTask = nil
            sessions = latest
        }
    }

    func detachedSessions(inWorktree key: WorktreeKey, ownedSessionIDs: Set<UUID>) -> [SessionDescriptor] {
        Self.detached(sessions, inWorktree: key, ownedSessionIDs: ownedSessionIDs)
    }

    nonisolated static func filter(_ sessions: [SessionDescriptor], inWorktree key: WorktreeKey) -> [SessionDescriptor] {
        sessions.filter { descriptor in
            descriptor.value(forMetadataKey: SessionMetadataKey.project) == key.projectID.uuidString
                && descriptor.value(forMetadataKey: SessionMetadataKey.worktree) == key.worktreeID.uuidString
        }
    }

    nonisolated static func detached(
        _ sessions: [SessionDescriptor],
        inWorktree key: WorktreeKey,
        ownedSessionIDs: Set<UUID>
    ) -> [SessionDescriptor] {
        filter(sessions, inWorktree: key).filter { descriptor in
            guard let sessionID = UUID(uuidString: descriptor.identifier.uuidString) else { return true }
            return !ownedSessionIDs.contains(sessionID)
        }
    }

    nonisolated static func displayTitle(for descriptor: SessionDescriptor) -> String {
        let title = descriptor.value(forMetadataKey: SessionMetadataKey.title)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard title.isEmpty else { return title }
        let directory = (descriptor.workingDirectory as NSString).lastPathComponent
        guard !directory.isEmpty, directory != "/" else { return TerminalPaneState.defaultTitle }
        return directory
    }
}
