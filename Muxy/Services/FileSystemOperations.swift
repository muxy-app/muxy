import AppKit
import Foundation

enum FileSystemOperationError: Error, Equatable {
    case destinationExists(String)
    case sourceMissing(String)
    case invalidName
    case sameAsSource
    case underlying(String)

    var userMessage: String {
        switch self {
        case let .destinationExists(path):
            "“\((path as NSString).lastPathComponent)” already exists"
        case let .sourceMissing(path):
            "“\((path as NSString).lastPathComponent)” no longer exists"
        case .invalidName:
            "That name is not allowed"
        case .sameAsSource:
            "Can’t move a folder into itself"
        case let .underlying(message):
            message
        }
    }
}

enum FileSystemOperations {
    static func writeFile(contents: String, atAbsolutePath absolutePath: String) async throws {
        try await GitProcessRunner.offMainThrowing {
            try writeFileSync(contents: contents, atAbsolutePath: absolutePath)
        }
    }

    nonisolated static func writeFileSync(contents: String, atAbsolutePath absolutePath: String) throws {
        do {
            try contents.write(toFile: absolutePath, atomically: true, encoding: .utf8)
        } catch {
            throw FileSystemOperationError.underlying(error.localizedDescription)
        }
    }

    static func moveToTrash(_ absolutePaths: [String]) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let urls = absolutePaths.map { URL(fileURLWithPath: $0) }
            NSWorkspace.shared.recycle(urls) { _, error in
                if let error {
                    continuation.resume(throwing: FileSystemOperationError.underlying(error.localizedDescription))
                } else {
                    continuation.resume(returning: ())
                }
            }
        }
    }

    static func isInside(path: String, ancestor: String) -> Bool {
        let normalized = ancestor.hasSuffix("/") ? String(ancestor.dropLast()) : ancestor
        return path == normalized || path.hasPrefix(normalized + "/")
    }

    nonisolated static func uniquePathSync(forName name: String, in directory: String) -> String {
        let fm = FileManager.default
        let baseURL = URL(fileURLWithPath: directory).appendingPathComponent(name)
        if !fm.fileExists(atPath: baseURL.path) {
            return baseURL.path
        }
        let ext = (name as NSString).pathExtension
        let stem = (name as NSString).deletingPathExtension
        var counter = 2
        while true {
            let candidateName = ext.isEmpty ? "\(stem) \(counter)" : "\(stem) \(counter).\(ext)"
            let candidate = URL(fileURLWithPath: directory).appendingPathComponent(candidateName)
            if !fm.fileExists(atPath: candidate.path) {
                return candidate.path
            }
            counter += 1
        }
    }

    nonisolated static func createFolderSync(named rawName: String, in directory: String) throws -> String {
        let name = try sanitize(rawName)
        let target = uniquePathSync(forName: name, in: directory)
        do {
            try FileManager.default.createDirectory(
                atPath: target,
                withIntermediateDirectories: false,
                attributes: nil
            )
        } catch {
            throw FileSystemOperationError.underlying(error.localizedDescription)
        }
        return target
    }

    nonisolated static func renameSync(at absolutePath: String, to rawName: String) throws -> String {
        let name = try sanitize(rawName)
        let parent = (absolutePath as NSString).deletingLastPathComponent
        let currentName = (absolutePath as NSString).lastPathComponent
        if name == currentName { return absolutePath }
        let candidate = URL(fileURLWithPath: parent).appendingPathComponent(name).path
        if FileManager.default.fileExists(atPath: candidate) {
            throw FileSystemOperationError.destinationExists(candidate)
        }
        do {
            try FileManager.default.moveItem(atPath: absolutePath, toPath: candidate)
        } catch {
            throw FileSystemOperationError.underlying(error.localizedDescription)
        }
        return candidate
    }

    nonisolated static func transferSync(
        sources: [String],
        destinationDirectory: String
    ) throws -> [String] {
        let fm = FileManager.default
        var results: [String] = []
        results.reserveCapacity(sources.count)
        for source in sources {
            guard fm.fileExists(atPath: source) else {
                throw FileSystemOperationError.sourceMissing(source)
            }
            let sourceParent = (source as NSString).deletingLastPathComponent
            let name = (source as NSString).lastPathComponent
            if sourceParent == destinationDirectory {
                results.append(source)
                continue
            }
            if isInside(path: destinationDirectory, ancestor: source) {
                throw FileSystemOperationError.sameAsSource
            }
            let target = uniquePathSync(forName: name, in: destinationDirectory)
            do {
                try fm.moveItem(atPath: source, toPath: target)
            } catch {
                throw FileSystemOperationError.underlying(error.localizedDescription)
            }
            results.append(target)
        }
        return results
    }

    nonisolated private static func sanitize(_ rawName: String) throws -> String {
        let trimmed = rawName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, !trimmed.contains("/"), trimmed != ".", trimmed != ".." else {
            throw FileSystemOperationError.invalidName
        }
        return trimmed
    }
}
