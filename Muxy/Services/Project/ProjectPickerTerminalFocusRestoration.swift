import Foundation

struct ProjectPickerTerminalFocusRestoration {
    private(set) var isPending = false

    mutating func record(_ result: ProjectOpenConfirmationResult) {
        guard result.didConfirm else { return }
        isPending = true
    }

    mutating func overlayExitCompleted(notificationCenter: NotificationCenter = .default) {
        guard isPending else { return }
        isPending = false
        notificationCenter.post(name: .refocusActiveTerminal, object: nil)
    }
}
