import Foundation

extension MuxyAPI {
    @MainActor
    enum Files {
        struct Context {
            let extensionID: String
            let appState: AppState
            let projectStore: ProjectStore
            let worktreeStore: WorktreeStore
            let projectGroupStore: ProjectGroupStore
        }

        static func list(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<[FileTreeEntry], APIError> {
            await read(projectIdentifier, context) { root in
                try await WorkspaceFileService.list(root: root, path: path)
            }
        }

        static func read(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<WorkspaceFileService.ReadResult, APIError> {
            await read(projectIdentifier, context) { root in
                try await WorkspaceFileService.read(root: root, path: path, encoding: .utf8)
            }
        }

        static func stat(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<WorkspaceFileService.StatResult, APIError> {
            await read(projectIdentifier, context) { root in
                try await WorkspaceFileService.stat(root: root, path: path)
            }
        }

        static func write(
            projectIdentifier: String?,
            path: String,
            contents: String,
            context: Context
        ) async -> Result<String, APIError> {
            await write(projectIdentifier, operation: "write", path: path, context: context) { root in
                try await WorkspaceFileService.write(root: root, path: path, contents: contents, encoding: .utf8)
            }
        }

        static func mkdir(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<String, APIError> {
            await write(projectIdentifier, operation: "mkdir", path: path, context: context) { root in
                try await WorkspaceFileService.mkdir(root: root, path: path)
            }
        }

        static func rename(
            projectIdentifier: String?,
            path: String,
            newName: String,
            context: Context
        ) async -> Result<String, APIError> {
            await write(projectIdentifier, operation: "rename", path: path, context: context) { root in
                try await WorkspaceFileService.rename(root: root, path: path, newName: newName)
            }
        }

        static func move(
            projectIdentifier: String?,
            paths: [String],
            into destination: String,
            context: Context
        ) async -> Result<[String], APIError> {
            await write(projectIdentifier, operation: "move", path: destination, context: context) { root in
                try await WorkspaceFileService.move(root: root, paths: paths, into: destination)
            }
        }

        static func delete(
            projectIdentifier: String?,
            paths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "delete", path: paths.first ?? "", context: context) { root in
                try await WorkspaceFileService.delete(root: root, paths: paths)
            }
        }

        private static func read<T: Sendable>(
            _ projectIdentifier: String?,
            _ context: Context,
            _ work: (WorkspaceFileService.Root) async throws -> T
        ) async -> Result<T, APIError> {
            guard let root = workspaceRoot(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            do {
                return try await .success(work(root))
            } catch {
                return .failure(.underlying(message(for: error)))
            }
        }

        private static func write<T: Sendable>(
            _ projectIdentifier: String?,
            operation: String,
            path: String,
            context: Context,
            _ work: (WorkspaceFileService.Root) async throws -> T
        ) async -> Result<T, APIError> {
            guard let root = workspaceRoot(projectIdentifier, context: context) else {
                return .failure(.projectNotFound(projectIdentifier ?? ""))
            }
            let consent = ExtensionConsentRequestBuilder.make(
                extensionID: context.extensionID,
                verb: .filesWrite,
                payload: .file(operation: operation, path: path),
                source: "muxy-api"
            )
            guard await ExtensionConsentService.shared.gate(consent) == .allow else {
                return .failure(.consentDenied(verb: "files.\(operation)"))
            }
            do {
                return try await .success(work(root))
            } catch {
                return .failure(.underlying(message(for: error)))
            }
        }

        nonisolated private static func message(for error: Error) -> String {
            if let operationError = error as? FileSystemOperationError {
                return operationError.userMessage
            }
            return error.localizedDescription
        }

        private static func workspaceRoot(
            _ projectIdentifier: String?,
            context: Context
        ) -> WorkspaceFileService.Root? {
            guard let project = context.projectGroupStore.resolveProject(
                identifier: projectIdentifier,
                localProjects: context.projectStore.projects,
                activeProjectID: context.appState.activeProjectID
            )
            else { return nil }
            return WorkspaceFileService.Root(
                path: activeWorktreePath(for: project.id, fallback: project.path, context: context),
                workspaceContext: context.projectGroupStore.workspaceContext(for: project)
            )
        }

        private static func activeWorktreePath(for projectID: UUID, fallback: String, context: Context) -> String {
            if let worktreeID = context.appState.activeWorktreeID[projectID],
               let worktree = context.worktreeStore.worktree(projectID: projectID, worktreeID: worktreeID)
            {
                return worktree.path
            }
            return fallback
        }
    }
}
