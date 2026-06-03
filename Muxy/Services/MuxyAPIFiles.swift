import Foundation

extension MuxyAPI {
    @MainActor
    enum Files {
        struct Context {
            let extensionID: String
            let appState: AppState
            let projectStore: ProjectStore
            let worktreeStore: WorktreeStore
        }

        nonisolated static let maxReadBytes = 5 * 1024 * 1024

        static func list(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<[FileTreeEntry], APIError> {
            await read(projectIdentifier, context) { root in
                let absolute = try contained(root: root, relativePath: path)
                return await FileTreeService.loadChildren(of: absolute, repoRoot: root)
            }
        }

        struct ReadResult {
            let relativePath: String
            let content: String
            let size: Int
        }

        static func read(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<ReadResult, APIError> {
            await read(projectIdentifier, context) { root in
                try await GitProcessRunner.offMainThrowing {
                    let absolute = try contained(root: root, relativePath: path)
                    let url = URL(fileURLWithPath: absolute)
                    let attributes = try FileManager.default.attributesOfItem(atPath: absolute)
                    let size = (attributes[.size] as? Int) ?? 0
                    guard size <= maxReadBytes else {
                        throw FileSystemOperationError.underlying("file exceeds \(maxReadBytes) byte read limit")
                    }
                    let data = try Data(contentsOf: url)
                    guard let content = String(data: data, encoding: .utf8) else {
                        throw FileSystemOperationError.underlying("file is not valid UTF-8 text")
                    }
                    return ReadResult(relativePath: relative(absolute, root: root), content: content, size: size)
                }
            }
        }

        struct StatResult {
            let name: String
            let relativePath: String
            let isDirectory: Bool
            let size: Int
        }

        static func stat(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<StatResult, APIError> {
            await read(projectIdentifier, context) { root in
                try await GitProcessRunner.offMainThrowing {
                    let absolute = try contained(root: root, relativePath: path)
                    var isDirectory: ObjCBool = false
                    guard FileManager.default.fileExists(atPath: absolute, isDirectory: &isDirectory) else {
                        throw FileSystemOperationError.sourceMissing(absolute)
                    }
                    let attributes = try FileManager.default.attributesOfItem(atPath: absolute)
                    return StatResult(
                        name: (absolute as NSString).lastPathComponent,
                        relativePath: relative(absolute, root: root),
                        isDirectory: isDirectory.boolValue,
                        size: (attributes[.size] as? Int) ?? 0
                    )
                }
            }
        }

        static func write(
            projectIdentifier: String?,
            path: String,
            contents: String,
            context: Context
        ) async -> Result<String, APIError> {
            await write(projectIdentifier, operation: "write", path: path, context: context) { root in
                try await GitProcessRunner.offMainThrowing {
                    let absolute = try contained(root: root, relativePath: path)
                    try FileSystemOperations.writeFileSync(contents: contents, atAbsolutePath: absolute)
                    return relative(absolute, root: root)
                }
            }
        }

        static func mkdir(
            projectIdentifier: String?,
            path: String,
            context: Context
        ) async -> Result<String, APIError> {
            await write(projectIdentifier, operation: "mkdir", path: path, context: context) { root in
                try await GitProcessRunner.offMainThrowing {
                    let absolute = try contained(root: root, relativePath: path)
                    let parent = (absolute as NSString).deletingLastPathComponent
                    let name = (absolute as NSString).lastPathComponent
                    let created = try FileSystemOperations.createFolderSync(named: name, in: parent)
                    return relative(created, root: root)
                }
            }
        }

        static func rename(
            projectIdentifier: String?,
            path: String,
            newName: String,
            context: Context
        ) async -> Result<String, APIError> {
            await write(projectIdentifier, operation: "rename", path: path, context: context) { root in
                let (source, moved) = try await GitProcessRunner.offMainThrowing {
                    let absolute = try contained(root: root, relativePath: path)
                    let moved = try FileSystemOperations.renameSync(at: absolute, to: newName)
                    return (absolute, moved)
                }
                context.appState.handleFileMoved(from: source, to: moved)
                return relative(moved, root: root)
            }
        }

        static func move(
            projectIdentifier: String?,
            paths: [String],
            into destination: String,
            context: Context
        ) async -> Result<[String], APIError> {
            await write(projectIdentifier, operation: "move", path: destination, context: context) { root in
                let (sources, moved) = try await GitProcessRunner.offMainThrowing {
                    let destinationAbsolute = try contained(root: root, relativePath: destination)
                    let sources = try paths.map { try contained(root: root, relativePath: $0) }
                    let moved = try FileSystemOperations.transferSync(
                        sources: sources,
                        destinationDirectory: destinationAbsolute
                    )
                    return (sources, moved)
                }
                for (source, target) in zip(sources, moved) {
                    context.appState.handleFileMoved(from: source, to: target)
                }
                return moved.map { relative($0, root: root) }
            }
        }

        static func delete(
            projectIdentifier: String?,
            paths: [String],
            context: Context
        ) async -> Result<Void, APIError> {
            await write(projectIdentifier, operation: "delete", path: paths.first ?? "", context: context) { root in
                let absolutes = try await GitProcessRunner.offMainThrowing {
                    try paths.map { try contained(root: root, relativePath: $0) }
                }
                try await FileSystemOperations.moveToTrash(absolutes)
            }
        }

        nonisolated static func contained(root: String, relativePath: String) throws -> String {
            guard let absolute = resolve(root: root, relativePath: relativePath) else {
                throw escapeError(relativePath)
            }
            return absolute
        }

        nonisolated static func resolve(root: String, relativePath: String) -> String? {
            let base = URL(fileURLWithPath: root).resolvingSymlinksInPath()
            let trimmed = relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath
            let resolved = canonicalize(base: base, relativePath: trimmed)
            guard isInside(resolved, base: base) else { return nil }
            return resolved.path
        }

        nonisolated private static func isInside(_ url: URL, base: URL) -> Bool {
            url.path == base.path || url.path.hasPrefix(base.path + "/")
        }

        nonisolated private static func canonicalize(base: URL, relativePath: String) -> URL {
            var current = base
            for component in relativePath.split(separator: "/", omittingEmptySubsequences: true).map(String.init) {
                if component == "." { continue }
                if component == ".." {
                    current = current.deletingLastPathComponent()
                    continue
                }
                current = follow(current.appendingPathComponent(component), from: current)
            }
            return current.standardizedFileURL
        }

        nonisolated private static func follow(_ candidate: URL, from parent: URL) -> URL {
            let attributes = try? FileManager.default.attributesOfItem(atPath: candidate.path)
            guard (attributes?[.type] as? FileAttributeType) == .typeSymbolicLink,
                  let destination = try? FileManager.default.destinationOfSymbolicLink(atPath: candidate.path)
            else { return candidate.standardizedFileURL }
            let resolved = destination.hasPrefix("/")
                ? URL(fileURLWithPath: destination)
                : parent.appendingPathComponent(destination)
            return resolved.standardizedFileURL
        }

        nonisolated private static func relative(_ absolute: String, root: String) -> String {
            let base = URL(fileURLWithPath: root).resolvingSymlinksInPath().path
            let normalized = URL(fileURLWithPath: absolute).standardizedFileURL.resolvingSymlinksInPath().path
            guard normalized.hasPrefix(base + "/") else { return (absolute as NSString).lastPathComponent }
            return String(normalized.dropFirst(base.count + 1))
        }

        nonisolated private static func escapeError(_ path: String) -> FileSystemOperationError {
            .underlying("path '\(path)' escapes the workspace root")
        }

        nonisolated private static func message(for error: Error) -> String {
            if let operationError = error as? FileSystemOperationError {
                return operationError.userMessage
            }
            return error.localizedDescription
        }

        private static func read<T: Sendable>(
            _ projectIdentifier: String?,
            _ context: Context,
            _ work: (String) async throws -> T
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
            _ work: (String) async throws -> T
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

        private static func workspaceRoot(_ projectIdentifier: String?, context: Context) -> String? {
            let project: Project? = if let projectIdentifier, !projectIdentifier.isEmpty {
                matchProject(projectIdentifier, in: context.projectStore.projects)
            } else if let activeProjectID = context.appState.activeProjectID {
                context.projectStore.projects.first { $0.id == activeProjectID }
            } else {
                nil
            }
            guard let project else { return nil }
            if let worktreeID = context.appState.activeWorktreeID[project.id],
               let worktree = context.worktreeStore.worktree(projectID: project.id, worktreeID: worktreeID)
            {
                return worktree.path
            }
            return project.path
        }

        private static func matchProject(_ identifier: String, in projects: [Project]) -> Project? {
            let standardizedPath = URL(fileURLWithPath: identifier).standardizedFileURL.path
            return projects.first { project in
                project.id.uuidString == identifier
                    || project.name.localizedCaseInsensitiveCompare(identifier) == .orderedSame
                    || URL(fileURLWithPath: project.path).standardizedFileURL.path == standardizedPath
            }
        }
    }
}
