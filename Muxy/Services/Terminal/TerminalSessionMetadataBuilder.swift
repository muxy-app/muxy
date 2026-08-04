import Foundation
import MuxySessionProtocol

@MainActor
enum TerminalSessionMetadataBuilder {
    static func build(
        worktreeKey: WorktreeKey,
        tabID: UUID?,
        title: String
    ) -> [(key: String, value: String)] {
        var entries: [(key: String, value: String)] = [
            (key: SessionMetadataKey.project, value: worktreeKey.projectID.uuidString),
            (key: SessionMetadataKey.worktree, value: worktreeKey.worktreeID.uuidString),
        ]
        if let tabID {
            entries.append((key: SessionMetadataKey.tab, value: tabID.uuidString))
        }
        let trimmedTitle = title.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedTitle.isEmpty {
            entries.append((key: SessionMetadataKey.title, value: trimmedTitle))
        }
        return entries
    }
}
