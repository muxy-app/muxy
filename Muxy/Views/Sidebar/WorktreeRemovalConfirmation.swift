import AppKit

struct WorktreeRemovalConfirmation {
    let title: String
    let message: String
    let style: NSAlert.Style

    init(worktree: Worktree, hasUncommittedChanges: Bool) {
        title = "Remove worktree \"\(worktree.name)\"?"
        if hasUncommittedChanges {
            message = "This worktree has uncommitted changes. Removing it will permanently discard them."
            style = .critical
            return
        }
        message = "This will remove the worktree from Muxy and delete its files on disk."
        style = .warning
    }
}

@MainActor
enum WorktreeRemovalConfirmationPresenter {
    static func present(
        worktree: Worktree,
        hasUncommittedChanges: Bool,
        onConfirm: @escaping () -> Void
    ) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let confirmation = WorktreeRemovalConfirmation(
            worktree: worktree,
            hasUncommittedChanges: hasUncommittedChanges
        )
        let alert = NSAlert()
        alert.messageText = confirmation.title
        alert.informativeText = confirmation.message
        alert.alertStyle = confirmation.style
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Remove")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = ""
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            guard response == .alertFirstButtonReturn else { return }
            onConfirm()
        }
    }
}
