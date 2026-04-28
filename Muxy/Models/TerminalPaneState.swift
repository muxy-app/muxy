import Foundation

@MainActor
@Observable
final class TerminalPaneState: Identifiable {
    let id = UUID()
    let projectPath: String
    var title: String
    let startupCommand: String?
    let startupCommandInteractive: Bool
    let externalEditorFilePath: String?
    let searchState = TerminalSearchState()
    @ObservationIgnored private var titleDebounceTask: Task<Void, Never>?

    init(
        projectPath: String,
        title: String = "Terminal",
        startupCommand: String? = nil,
        startupCommandInteractive: Bool = false,
        externalEditorFilePath: String? = nil
    ) {
        self.projectPath = projectPath
        self.title = title
        self.startupCommand = startupCommand
        self.startupCommandInteractive = startupCommandInteractive
        self.externalEditorFilePath = externalEditorFilePath
    }

    func setTitle(_ newTitle: String) {
        titleDebounceTask?.cancel()
        titleDebounceTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled, let self, self.title != newTitle else { return }
            self.title = newTitle
        }
    }
}
