import Foundation

struct WorktreeRemovalConfirmation: Identifiable, Equatable {
    let worktree: Worktree
    let title: LocalizedStringResource
    let message: String
    let teardownCommands: [WorktreeConfig.ResolvedCommand]
    let projectHookApproval: WorktreeConfig.ProjectHookApproval?

    var id: UUID {
        worktree.id
    }

    @MainActor
    init(
        worktree: Worktree,
        hasUncommittedChanges: Bool,
        teardownCommands: [WorktreeConfig.ResolvedCommand] = [],
        approvesProjectHooks: Bool = false,
        stopsLocalProcesses: Bool = true
    ) {
        self.worktree = worktree
        title = "Remove worktree \"\(worktree.name)\"?"
        let normalizedCommands = WorktreeConfig.normalizedCommands(teardownCommands)
        self.teardownCommands = normalizedCommands
        projectHookApproval = approvesProjectHooks
            ? WorktreeConfig.ProjectHookApproval(resolvedCommands: normalizedCommands)
            : nil
        message = Self.message(
            hasUncommittedChanges: hasUncommittedChanges,
            teardownCommands: normalizedCommands,
            stopsLocalProcesses: stopsLocalProcesses
        )
    }

    @MainActor
    static func prepare(
        worktree: Worktree,
        projectPath: String,
        context: WorkspaceContext
    ) async throws -> WorktreeRemovalConfirmation {
        let hasChanges = await GitWorktreeService.shared.hasUncommittedChanges(
            worktreePath: worktree.path,
            context: context
        )
        guard !context.isRemote, !worktree.isExternallyManaged else {
            return WorktreeRemovalConfirmation(
                worktree: worktree,
                hasUncommittedChanges: hasChanges,
                stopsLocalProcesses: !context.isRemote
            )
        }
        let commands = try WorktreeConfig.resolvedTeardownCommands(
            sourceProjectPath: projectPath,
            globalConfigURL: WorktreeConfig.globalConfigURL()
        )
        return WorktreeRemovalConfirmation(
            worktree: worktree,
            hasUncommittedChanges: hasChanges,
            teardownCommands: commands,
            approvesProjectHooks: true,
            stopsLocalProcesses: true
        )
    }

    @MainActor
    private static func message(
        hasUncommittedChanges: Bool,
        teardownCommands: [WorktreeConfig.ResolvedCommand],
        stopsLocalProcesses: Bool
    ) -> String {
        let removalMessage = hasUncommittedChanges
            ? L10n.string("This worktree has uncommitted changes. Removing it will permanently discard them.")
            : L10n.string("This will remove the worktree from Muxy and delete its files on disk.")
        let processMessage = stopsLocalProcesses
            ? L10n.string("Local processes running from this worktree will be stopped.")
            : nil
        guard !teardownCommands.isEmpty else {
            return [removalMessage, processMessage].compactMap(\.self).joined(separator: "\n\n")
        }
        let heading = L10n.string("The following teardown commands will run before removal:")
        let commands = teardownCommands.map { command in
            let source = switch command.source {
            case .global: L10n.string("Per-machine")
            case .project: L10n.string("Project")
            }
            return "\(source): \(command.command.command)"
        }
        return ([removalMessage, processMessage, heading].compactMap(\.self) + commands).joined(separator: "\n\n")
    }
}

enum WorktreeRemovalRequestPolicy {
    struct ConfirmationConditions {
        let expected: WorktreeKey
        let current: WorktreeKey?
        let isRegistered: Bool
        let isPreparing: Bool
        let isRemoving: Bool
        let hasPendingConfirmation: Bool
    }

    static func canStartInspection(
        hasPendingConfirmation: Bool,
        isInspecting: Bool,
        isRemoving: Bool
    ) -> Bool {
        !hasPendingConfirmation && !isInspecting && !isRemoving
    }

    static func canPresentConfirmation(_ conditions: ConfirmationConditions) -> Bool {
        conditions.expected == conditions.current
            && conditions.isRegistered
            && conditions.isPreparing
            && !conditions.isRemoving
            && !conditions.hasPendingConfirmation
    }
}
