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
    static func createFile(named rawName: String, in directory: String) async throws -> String {
        try await GitProcessRunner.offMainThrowing {
            try createFileSync(named: rawName, in: directory)
        }
    }

    static func createFolder(named rawName: String, in directory: String) async throws -> String {
        try await GitProcessRunner.offMainThrowing {
            try createFolderSync(named: rawName, in: directory)
        }
    }

    static func rename(at absolutePath: String, to rawName: String) async throws -> String {
        try await GitProcessRunner.offMainThrowing {
            try renameSync(at: absolutePath, to: rawName)
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

    static func move(_ sources: [String], into destinationDirectory: String) async throws -> [String] {
        try await GitProcessRunner.offMainThrowing {
            try transferSync(sources: sources, destinationDirectory: destinationDirectory, copy: false)
        }
    }

    static func copy(_ sources: [String], into destinationDirectory: String) async throws -> [String] {
        try await GitProcessRunner.offMainThrowing {
            try transferSync(sources: sources, destinationDirectory: destinationDirectory, copy: true)
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

    nonisolated private static func createFileSync(named rawName: String, in directory: String) throws -> String {
        let name = try sanitize(rawName)
        let target = uniquePathSync(forName: name, in: directory)
        let created = FileManager.default.createFile(atPath: target, contents: nil)
        guard created else {
            throw FileSystemOperationError.underlying("Failed to create file at \(target)")
        }
        return target
    }

    nonisolated private static func createFolderSync(named rawName: String, in directory: String) throws -> String {
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

    nonisolated private static func renameSync(at absolutePath: String, to rawName: String) throws -> String {
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

    nonisolated private static func transferSync(
        sources: [String],
        destinationDirectory: String,
        copy: Bool
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
            if !copy, sourceParent == destinationDirectory {
                results.append(source)
                continue
            }
            if !copy, isInside(path: destinationDirectory, ancestor: source) {
                throw FileSystemOperationError.sameAsSource
            }
            let target = uniquePathSync(forName: name, in: destinationDirectory)
            do {
                if copy {
                    try fm.copyItem(atPath: source, toPath: target)
                } else {
                    try fm.moveItem(atPath: source, toPath: target)
                }
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
