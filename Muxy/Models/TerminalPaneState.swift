import Foundation

@MainActor
@Observable
final class TerminalPaneState: Identifiable {
    enum WorkingDirectoryMode: String, Codable {
        case projectRoot
        case rememberLast
        case fixedDefault
    }

    let id = UUID()
    let projectPath: String
    var title: String = "Terminal"
    var workingDirectoryMode: WorkingDirectoryMode = .projectRoot
    var lastKnownWorkingDirectory: String?
    var defaultWorkingDirectory: String?
    let searchState = TerminalSearchState()

    init(projectPath: String) {
        self.projectPath = projectPath
    }

    init(
        projectPath: String,
        title: String,
        workingDirectoryMode: WorkingDirectoryMode = .projectRoot,
        lastKnownWorkingDirectory: String? = nil,
        defaultWorkingDirectory: String? = nil
    ) {
        self.projectPath = projectPath
        self.title = title
        self.workingDirectoryMode = workingDirectoryMode
        self.lastKnownWorkingDirectory = lastKnownWorkingDirectory
        self.defaultWorkingDirectory = defaultWorkingDirectory
    }

    var initialWorkingDirectory: String {
        let candidate: String? = switch workingDirectoryMode {
        case .projectRoot:
            projectPath
        case .rememberLast:
            lastKnownWorkingDirectory ?? projectPath
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
