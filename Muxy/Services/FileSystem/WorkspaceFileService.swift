import Foundation
import MuxyShared

@MainActor
enum WorkspaceFileService {
    struct Root {
        let path: String
        let workspaceContext: WorkspaceContext

        var remoteFileService: RemoteFileService? { workspaceContext.remoteFileService }
    }

    struct ReadResult: Sendable {
        let relativePath: String
        let content: String
        let size: Int
        let encoding: FileEncodingDTO
    }

    struct StatResult: Sendable {
        let name: String
        let relativePath: String
        let isDirectory: Bool
        let size: Int
    }

    nonisolated static let maxReadBytes = 5 * 1024 * 1024

    static func list(root: Root, path: String) async throws -> [FileTreeEntry] {
        if let remote = root.remoteFileService {
            return try await remote.list(root: root.path, relativePath: path)
        }
        let absolute = try contained(root: root.path, relativePath: path)
        return await FileTreeService.loadChildren(of: absolute, repoRoot: root.path)
    }

    static func read(root: Root, path: String, encoding: FileEncodingDTO) async throws -> ReadResult {
        if let remote = root.remoteFileService {
            return try await remote.read(
                root: root.path,
                relativePath: path,
                maxBytes: maxReadBytes,
                encoding: encoding
            )
        }
        return try await GitProcessRunner.offMainThrowing {
            let absolute = try contained(root: root.path, relativePath: path)
            let attributes = try FileManager.default.attributesOfItem(atPath: absolute)
            let size = (attributes[.size] as? Int) ?? 0
            guard size <= maxReadBytes else {
                throw FileSystemOperationError.underlying("file exceeds \(maxReadBytes) byte read limit")
            }
            let data = try Data(contentsOf: URL(fileURLWithPath: absolute))
            return try ReadResult(
                relativePath: relative(absolute, root: root.path),
                content: encode(data, as: encoding),
                size: size,
                encoding: encoding
            )
        }
    }

    static func stat(root: Root, path: String) async throws -> StatResult {
        if let remote = root.remoteFileService {
            return try await remote.stat(root: root.path, relativePath: path)
        }
        return try await GitProcessRunner.offMainThrowing {
            let absolute = try contained(root: root.path, relativePath: path)
            var isDirectory: ObjCBool = false
            guard FileManager.default.fileExists(atPath: absolute, isDirectory: &isDirectory) else {
                throw FileSystemOperationError.sourceMissing(absolute)
            }
            let attributes = try FileManager.default.attributesOfItem(atPath: absolute)
            return StatResult(
                name: (absolute as NSString).lastPathComponent,
                relativePath: relative(absolute, root: root.path),
                isDirectory: isDirectory.boolValue,
                size: (attributes[.size] as? Int) ?? 0
            )
        }
    }

    static func write(
        root: Root,
        path: String,
        contents: String,
        encoding: FileEncodingDTO
    ) async throws -> String {
        if let remote = root.remoteFileService {
            return try await remote.write(
                root: root.path,
                relativePath: path,
                contents: contents,
                encoding: encoding
            )
        }
        return try await GitProcessRunner.offMainThrowing {
            let absolute = try contained(root: root.path, relativePath: path)
            switch encoding {
            case .utf8:
                try FileSystemOperations.writeFileSync(contents: contents, atAbsolutePath: absolute)
            case .base64:
                try FileSystemOperations.writeFileSync(data: decodeBase64(contents), atAbsolutePath: absolute)
            }
            return relative(absolute, root: root.path)
        }
    }

    static func mkdir(root: Root, path: String) async throws -> String {
        if let remote = root.remoteFileService {
            return try await remote.mkdir(root: root.path, relativePath: path)
        }
        return try await GitProcessRunner.offMainThrowing {
            let absolute = try contained(root: root.path, relativePath: path)
            let parent = (absolute as NSString).deletingLastPathComponent
            let name = (absolute as NSString).lastPathComponent
            let created = try FileSystemOperations.createFolderSync(named: name, in: parent)
            return relative(created, root: root.path)
        }
    }

    static func rename(root: Root, path: String, newName: String) async throws -> String {
        if let remote = root.remoteFileService {
            return try await remote.rename(root: root.path, relativePath: path, newName: newName)
        }
        let moved = try await GitProcessRunner.offMainThrowing {
            let absolute = try contained(root: root.path, relativePath: path)
            return try FileSystemOperations.renameSync(at: absolute, to: newName)
        }
        return relative(moved, root: root.path)
    }

    static func move(root: Root, paths: [String], into destination: String) async throws -> [String] {
        if let remote = root.remoteFileService {
            return try await remote.move(root: root.path, paths: paths, into: destination)
        }
        let moved = try await GitProcessRunner.offMainThrowing {
            let destinationAbsolute = try contained(root: root.path, relativePath: destination)
            let sources = try paths.map { try contained(root: root.path, relativePath: $0) }
            return try FileSystemOperations.transferSync(
                sources: sources,
                destinationDirectory: destinationAbsolute
            )
        }
        return moved.map { relative($0, root: root.path) }
    }

    static func delete(root: Root, paths: [String]) async throws {
        if let remote = root.remoteFileService {
            return try await remote.delete(root: root.path, paths: paths)
        }
        let absolutes = try await GitProcessRunner.offMainThrowing {
            try paths.map { try contained(root: root.path, relativePath: $0) }
        }
        try await FileSystemOperations.moveToTrash(absolutes)
    }

    nonisolated static func encode(_ data: Data, as encoding: FileEncodingDTO) throws -> String {
        switch encoding {
        case .base64:
            return data.base64EncodedString()
        case .utf8:
            guard let content = String(data: data, encoding: .utf8) else {
                throw FileSystemOperationError.underlying("file is not valid UTF-8 text")
            }
            return content
        }
    }

    nonisolated static func decodeBase64(_ contents: String) throws -> Data {
        let stripped = contents.filter { !$0.isWhitespace }
        guard let data = Data(base64Encoded: stripped) else {
            throw FileSystemOperationError.underlying("contents are not valid base64")
        }
        return data
    }

    nonisolated static func contained(root: String, relativePath: String) throws -> String {
        guard let absolute = resolve(root: root, relativePath: relativePath) else {
            throw FileSystemOperationError.outsideRoot(relativePath)
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

    nonisolated static func relative(_ absolute: String, root: String) -> String {
        let base = URL(fileURLWithPath: root).resolvingSymlinksInPath().path
        let normalized = URL(fileURLWithPath: absolute).standardizedFileURL.resolvingSymlinksInPath().path
        guard normalized.hasPrefix(base + "/") else { return (absolute as NSString).lastPathComponent }
        return String(normalized.dropFirst(base.count + 1))
    }

    nonisolated private static func isInside(_ url: URL, base: URL) -> Bool {
        url.path == base.path || url.path.hasPrefix(base.path + "/")
    }

    nonisolated private static func canonicalize(base: URL, relativePath: String) -> URL {
        var current = base
        for component in relativePath.split(separator: "/", omittingEmptySubsequences: true).map(String.init) {
            if component == "." {
                continue
            }
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
}
