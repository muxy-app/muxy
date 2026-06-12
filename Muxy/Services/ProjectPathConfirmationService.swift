import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectPathConfirmation")

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
    let projectGroupStore: ProjectGroupStore
    let fileSystem: any ProjectPathConfirmationFileSystem

    init(
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore,
        fileSystem: any ProjectPathConfirmationFileSystem = FileManagerProjectPathConfirmationFileSystem()
    ) {
        self.appState = appState
        self.projectStore = projectStore
        self.worktreeStore = worktreeStore
        self.projectGroupStore = projectGroupStore
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
        projectGroupStore.addProjectToActiveGroup(projectID: project.id)
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

@MainActor
struct RemoteDeviceProjectConfirmationService {
    let appState: AppState
    let projectStore: ProjectStore
    let worktreeStore: WorktreeStore
    let projectGroupStore: ProjectGroupStore

    @discardableResult
    func confirm(path: String, device: RemoteDevice) -> ProjectOpenConfirmationResult {
        let standardizedPath = ProjectPickerPathService.standardizedRemotePath(path)
        logger.info(
            "Remote device confirm begin deviceID=\(device.id.uuidString, privacy: .public) host=\(device.ssh.host, privacy: .public) path=\(path, privacy: .public) standardizedPath=\(standardizedPath, privacy: .public) remoteRoot=\(device.ssh.remoteRoot, privacy: .public)"
        )
        guard standardizedPath != ProjectPickerPathService.standardizedRemotePath(device.ssh.remoteRoot) else {
            logger.info(
                "Remote device confirm rejected deviceID=\(device.id.uuidString, privacy: .public) reason=deviceRoot"
            )
            return .failed
        }

        let project = project(at: path, standardizedPath: standardizedPath, device: device)
        projectGroupStore.addProjectToActiveGroup(projectID: project.id)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return .failed }
        appState.selectProject(project, worktree: primary)
        logger.info(
            "Remote device confirm selected deviceID=\(device.id.uuidString, privacy: .public) projectID=\(project.id.uuidString, privacy: .public) activeGroupID=\(projectGroupStore.activeGroupID?.uuidString ?? "nil", privacy: .public) storedProjectCount=\(projectStore.storedProjects.count)"
        )
        return .success
    }

    private func project(at path: String, standardizedPath: String, device: RemoteDevice) -> Project {
        if let existing = projectStore.storedProjects.first(where: {
            $0.remoteDeviceID == device.id
                && ProjectPickerPathService.standardizedRemotePath($0.path) == standardizedPath
        }) {
            logger.info(
                "Remote device confirm reused existing projectID=\(existing.id.uuidString, privacy: .public) deviceID=\(device.id.uuidString, privacy: .public)"
            )
            return existing
        }

        let name = path.split(separator: "/").last.map(String.init) ?? path
        let project = Project(
            name: name,
            path: path,
            sortOrder: projectStore.storedProjects.count,
            remoteDeviceID: device.id
        )
        projectStore.add(project)
        logger.info(
            "Remote device confirm created projectID=\(project.id.uuidString, privacy: .public) deviceID=\(device.id.uuidString, privacy: .public) name=\(project.name, privacy: .public) path=\(project.path, privacy: .public)"
        )
        return project
    }
}
