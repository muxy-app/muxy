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
    nonisolated static let maxWriteBytes = 5 * 1024 * 1024

    static func list(root: Root, path: String) async throws -> [FileTreeEntry] {
        if let remote = root.remoteFileService {
            return try await remote.list(root: root.path, relativePath: path)
        }
        let absolute = try contained(root: root.path, relativePath: path)
        let repoRoot = try contained(root: root.path, relativePath: "")
        return try await FileTreeService.loadChildren(of: absolute, repoRoot: repoRoot)
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
            guard FileManager.default.fileExists(atPath: absolute) else {
                throw FileSystemOperationError.sourceMissing(absolute)
            }
            let handle = try FileHandle(forReadingFrom: URL(fileURLWithPath: absolute))
            defer { try? handle.close() }
            var data = Data()
            data.reserveCapacity(maxReadBytes + 1)
            while data.count <= maxReadBytes {
                let readCount = min(64 * 1024, maxReadBytes + 1 - data.count)
                let chunk = try handle.read(upToCount: readCount) ?? Data()
                guard !chunk.isEmpty else { break }
                data.append(chunk)
            }
            guard data.count <= maxReadBytes else {
                throw FileSystemOperationError.underlying("file exceeds \(maxReadBytes) byte read limit")
            }
            return try ReadResult(
                relativePath: relative(absolute, root: root.path),
                content: encode(data, as: encoding),
                size: data.count,
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
        let data = try await GitProcessRunner.offMainThrowing {
            let decoded = try decode(contents, as: encoding)
            guard decoded.count <= maxWriteBytes else {
                throw FileSystemOperationError.underlying("file exceeds \(maxWriteBytes) byte write limit")
            }
            return decoded
        }
        if let remote = root.remoteFileService {
            return try await remote.write(
                root: root.path,
                relativePath: path,
                data: data,
                maxBytes: maxWriteBytes
            )
        }
        return try await GitProcessRunner.offMainThrowing {
            let absolute = try containedMutation(root: root.path, relativePath: path)
            try FileSystemOperations.writeFileSync(data: data, atAbsolutePath: absolute)
            return relative(absolute, root: root.path)
        }
    }

    static func mkdir(root: Root, path: String) async throws -> String {
        if let remote = root.remoteFileService {
            return try await remote.mkdir(root: root.path, relativePath: path)
        }
        return try await GitProcessRunner.offMainThrowing {
            let absolute = try containedMutation(root: root.path, relativePath: path)
            let parent = (absolute as NSString).deletingLastPathComponent
            let name = (absolute as NSString).lastPathComponent
            let created = try FileSystemOperations.createFolderSync(named: name, in: parent)
            return try relative(
                containedAbsoluteMutation(root: root.path, absolutePath: created),
                root: root.path
            )
        }
    }

    static func rename(root: Root, path: String, newName: String) async throws -> String {
        if let remote = root.remoteFileService {
            return try await remote.rename(root: root.path, relativePath: path, newName: newName)
        }
        let moved = try await GitProcessRunner.offMainThrowing {
            let absolute = try containedMutation(root: root.path, relativePath: path)
            let target = try FileSystemOperations.renameSync(at: absolute, to: newName)
            return try containedAbsoluteMutation(root: root.path, absolutePath: target)
        }
        return relative(moved, root: root.path)
    }

    static func move(root: Root, paths: [String], into destination: String) async throws -> [String] {
        if let remote = root.remoteFileService {
            return try await remote.move(root: root.path, paths: paths, into: destination)
        }
        let moved = try await GitProcessRunner.offMainThrowing {
            let destinationAbsolute = try contained(root: root.path, relativePath: destination)
            let sources = try paths.map { try containedMutation(root: root.path, relativePath: $0) }
            let results = try FileSystemOperations.transferSync(
                sources: sources,
                destinationDirectory: destinationAbsolute
            )
            return try results.map {
                try containedAbsoluteMutation(root: root.path, absolutePath: $0)
            }
        }
        return moved.map { relative($0, root: root.path) }
    }

    static func delete(root: Root, paths: [String]) async throws {
        if let remote = root.remoteFileService {
            return try await remote.delete(root: root.path, paths: paths)
        }
        let absolutes = try await GitProcessRunner.offMainThrowing {
            try paths.map { try containedMutation(root: root.path, relativePath: $0) }
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

    nonisolated static func decode(_ contents: String, as encoding: FileEncodingDTO) throws -> Data {
        switch encoding {
        case .utf8:
            Data(contents.utf8)
        case .base64:
            try decodeBase64(contents)
        }
    }

    nonisolated static func contained(root: String, relativePath: String) throws -> String {
        guard let absolute = resolve(root: root, relativePath: relativePath) else {
            throw FileSystemOperationError.outsideRoot(relativePath)
        }
        return absolute
    }

    nonisolated static func containedMutation(root: String, relativePath: String) throws -> String {
        let absolute = try contained(root: root, relativePath: relativePath)
        return try requireMutationTarget(absolute, root: root, requestedPath: relativePath)
    }

    nonisolated static func resolve(root: String, relativePath: String) -> String? {
        let logicalBase = URL(fileURLWithPath: root).standardizedFileURL.path
        guard let physicalBase = canonicalizeAbsolute(root) else { return nil }
        let trimmed = relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath
        guard let resolved = canonicalize(base: physicalBase, relativePath: trimmed) else { return nil }
        guard isInside(resolved, base: physicalBase) else { return nil }
        guard resolved.path != physicalBase.path else { return logicalBase }
        let prefix = physicalBase.path == "/" ? "/" : physicalBase.path + "/"
        let relativePath = String(resolved.path.dropFirst(prefix.count))
        return (logicalBase as NSString).appendingPathComponent(relativePath)
    }

    nonisolated static func relative(_ absolute: String, root: String) -> String {
        let base = canonicalizeAbsolute(root)?.path ?? URL(fileURLWithPath: root).standardizedFileURL.path
        let normalized = canonicalizeAbsolute(absolute)?.path
            ?? URL(fileURLWithPath: absolute).standardizedFileURL.path
        guard normalized != base else { return "" }
        guard base != "/" else { return String(normalized.dropFirst()) }
        guard normalized.hasPrefix(base + "/") else { return (absolute as NSString).lastPathComponent }
        return String(normalized.dropFirst(base.count + 1))
    }

    nonisolated private static func isInside(_ url: URL, base: URL) -> Bool {
        if base.path == "/" {
            return url.path.hasPrefix("/")
        }
        return url.path == base.path || url.path.hasPrefix(base.path + "/")
    }

    nonisolated private static func containedAbsoluteMutation(
        root: String,
        absolutePath: String
    ) throws -> String {
        guard let base = canonicalizeAbsolute(root),
              let lexical = canonicalizeAbsolute(absolutePath)
        else {
            throw FileSystemOperationError.outsideRoot(absolutePath)
        }
        guard isInside(lexical, base: base) else {
            throw FileSystemOperationError.outsideRoot(absolutePath)
        }
        let relativePath = lexical.path == base.path
            ? ""
            : String(lexical.path.dropFirst(base.path.count + 1))
        return try containedMutation(root: root, relativePath: relativePath)
    }

    nonisolated private static func requireMutationTarget(
        _ absolute: String,
        root: String,
        requestedPath: String
    ) throws -> String {
        guard let base = canonicalizeAbsolute(root)?.path else {
            throw FileSystemOperationError.outsideRoot(requestedPath)
        }
        guard canonicalizeAbsolute(absolute)?.path != base else {
            throw FileSystemOperationError.outsideRoot(requestedPath)
        }
        return absolute
    }

    nonisolated private static func canonicalizeAbsolute(_ path: String) -> URL? {
        canonicalize(base: URL(fileURLWithPath: "/"), relativePath: path)
    }

    nonisolated private static func canonicalize(base: URL, relativePath: String) -> URL? {
        var current = base
        var pending = Array(pathComponents(relativePath).reversed())
        var states: Set<ResolutionState> = []
        var followedLinkCount = 0
        while let component = pending.popLast() {
            let state = ResolutionState(
                currentPath: current.path,
                pending: pending + [component]
            )
            guard states.insert(state).inserted else { return nil }
            if component == "." {
                continue
            }
            if component == ".." {
                current = current.deletingLastPathComponent()
                continue
            }
            let candidate = current.appendingPathComponent(component)
            let attributes = try? FileManager.default.attributesOfItem(atPath: candidate.path)
            guard (attributes?[.type] as? FileAttributeType) == .typeSymbolicLink else {
                current = candidate
                continue
            }
            followedLinkCount += 1
            guard followedLinkCount <= 64,
                  let destination = try? FileManager.default.destinationOfSymbolicLink(atPath: candidate.path)
            else {
                return nil
            }
            if destination.hasPrefix("/") {
                current = URL(fileURLWithPath: "/")
            }
            pending.append(contentsOf: pathComponents(destination).reversed())
        }
        return current
    }

    nonisolated private static func pathComponents(_ path: String) -> [String] {
        path.split(separator: "/", omittingEmptySubsequences: true).map(String.init)
    }

    private struct ResolutionState: Hashable {
        let currentPath: String
        let pending: [String]
    }
}
