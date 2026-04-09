import Foundation

@MainActor
@Observable
final class TerminalPaneState: Identifiable {
    enum WorkingDirectoryMode: String, Codable {
        case projectRoot
        case fixedDefault
    }

    let id = UUID()
    let projectPath: String
    var title: String = "Terminal"
    var workingDirectoryMode: WorkingDirectoryMode = .projectRoot
    var defaultWorkingDirectory: String?
    let searchState = TerminalSearchState()

    init(projectPath: String) {
        self.projectPath = projectPath
    }

    init(
        projectPath: String,
        title: String,
        workingDirectoryMode: WorkingDirectoryMode = .projectRoot,
        defaultWorkingDirectory: String? = nil
    ) {
        self.projectPath = projectPath
        self.title = title
        self.workingDirectoryMode = workingDirectoryMode
        self.defaultWorkingDirectory = defaultWorkingDirectory
    }

    var initialWorkingDirectory: String {
        let candidate: String? = switch workingDirectoryMode {
        case .projectRoot:
            projectPath
        case .fixedDefault:
            defaultWorkingDirectory ?? projectPath
        }
        return Self.isUsableDirectory(candidate) ? candidate ?? projectPath : projectPath
    }

    private static func isUsableDirectory(_ path: String?) -> Bool {
        guard let path, !path.isEmpty else { return false }
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &isDirectory) && isDirectory.boolValue
    }
}
