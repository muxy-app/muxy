import Foundation

@MainActor
@Observable
final class TerminalPaneState: Identifiable {
    let id = UUID()
    let projectPath: String
    var title: String = "Terminal"

    init(projectPath: String) {
        self.projectPath = projectPath
    }

    init(projectPath: String, title: String) {
        self.projectPath = projectPath
        self.title = title
    }
}
