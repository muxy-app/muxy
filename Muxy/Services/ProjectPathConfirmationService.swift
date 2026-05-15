import Foundation

enum ProjectOpenConfirmationResult: Equatable {
    case success
    case missingDirectory
    case notDirectory
    case createFailed
    case failed

    var didConfirm: Bool {
        self == .success
    }
}

enum ProjectPathConfirmationDirectoryState: Equatable {
    case missing
    case directory
    case notDirectory
}

protocol ProjectPathConfirmationFileSystem {
    func directoryState(atPath path: String) -> ProjectPathConfirmationDirectoryState
    func createDirectory(atPath path: String) throws
}

struct FileManagerProjectPathConfirmationFileSystem: ProjectPathConfirmationFileSystem {
    var fileManager: FileManager = .default

    func directoryState(atPath path: String) -> ProjectPathConfirmationDirectoryState {
        var isDirectory = ObjCBool(false)
        guard fileManager.fileExists(atPath: path, isDirectory: &isDirectory) else {
            return .missing
        }
        return isDirectory.boolValue ? .directory : .notDirectory
    }

    func createDirectory(atPath path: String) throws {
        try fileManager.createDirectory(
            at: URL(fileURLWithPath: path),
            withIntermediateDirectories: true
        )
    }
}

@MainActor
struct ProjectPathConfirmationService {
    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let fileSystem: any ProjectPathConfirmationFileSystem

    init(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        fileSystem: any ProjectPathConfirmationFileSystem = FileManagerProjectPathConfirmationFileSystem()
    ) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.fileSystem = fileSystem
    }

    @discardableResult
    func confirm(
        path: String,
        createIfMissing: Bool = false
    ) -> ProjectOpenConfirmationResult {
        let standardizedPath = ProjectPickerPathService.standardizedPath(path)
        if let failure = ensureDirectory(at: standardizedPath, createIfMissing: createIfMissing) {
            return failure
        }

        let project = project(at: standardizedPath)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return .failed }
        appState.selectProject(project, worktree: primary)
        return .success
    }

    private func ensureDirectory(
        at path: String,
        createIfMissing: Bool
    ) -> ProjectOpenConfirmationResult? {
        switch fileSystem.directoryState(atPath: path) {
        case .directory:
            return nil
        case .notDirectory:
            return .notDirectory
        case .missing:
            guard createIfMissing else { return .missingDirectory }
            do {
                try fileSystem.createDirectory(atPath: path)
            } catch {
                return .createFailed
            }
            return fileSystem.directoryState(atPath: path) == .directory ? nil : .failed
        }
    }

    private func project(at standardizedPath: String) -> Project {
        if let existing = projectStore.projects.first(where: {
            ProjectPickerPathService.standardizedPath($0.path) == standardizedPath
        }) {
            return existing
        }

        let url = URL(fileURLWithPath: standardizedPath)
        let project = Project(
            name: url.lastPathComponent,
            path: standardizedPath,
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        return project
    }
}
