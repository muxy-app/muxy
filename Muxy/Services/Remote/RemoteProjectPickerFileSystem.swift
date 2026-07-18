import Foundation

struct RemoteProjectPickerFileSystem: ProjectPickerFileSystem {
    let destination: SSHDestination
    private let cache = RemoteDirectoryCache()

    func directoryState(atPath path: String) -> ProjectPickerFileSystemDirectoryState {
        cache.directoryState(forPath: ProjectPickerPathService.standardizedRemotePath(path))
    }

    func isReadableFile(atPath path: String) -> Bool {
        directoryState(atPath: path) != .missing
    }

    func contentsOfDirectory(atPath path: String) async throws -> [ProjectPickerFileSystemDirectoryEntry] {
        let standardized = ProjectPickerPathService.standardizedRemotePath(path)
        let quoted = RemoteCommandBuilder.quoteRemotePath(standardized)
        let script = "cd \(quoted) && for e in * .*; do "
            + "case \"$e\" in '.'|'..'|'*'|'.*') continue ;; esac; "
            + "{ [ -e \"$e\" ] || [ -L \"$e\" ]; } || continue; "
            + "if [ -d \"$e\" ]; then printf 'd %s\\0' \"$e\"; "
            + "else printf 'f %s\\0' \"$e\"; fi; done"
        guard let result = try? await SSHCommandRunner.run(destination: destination, remoteCommand: script),
              result.status == 0
        else {
            throw RemoteProjectPickerError.listingFailed
        }
        let entries = Self.parseEntries(result.stdout)
        cache.store(directory: standardized, entries: entries)
        return entries
    }

    static func parseEntries(_ output: String) -> [ProjectPickerFileSystemDirectoryEntry] {
        output
            .split(separator: "\0", omittingEmptySubsequences: true)
            .compactMap { record -> ProjectPickerFileSystemDirectoryEntry? in
                guard record.count > 2 else { return nil }
                let type = record.first
                let name = String(record.dropFirst(2))
                guard !name.isEmpty else { return nil }
                return type == "d" ? .directory(name) : .file(name)
            }
    }
}

private final class RemoteDirectoryCache: @unchecked Sendable {
    private let lock = NSLock()
    private var entriesByDirectory: [String: Set<String>] = [:]
    private var directoriesByParent: [String: Set<String>] = [:]

    func store(directory: String, entries: [ProjectPickerFileSystemDirectoryEntry]) {
        lock.lock()
        defer { lock.unlock() }
        entriesByDirectory[directory] = Set(entries.map(\.name))
        directoriesByParent[directory] = Set(entries.filter(\.isProjectPickerDirectory).map(\.name))
    }

    func directoryState(forPath path: String) -> ProjectPickerFileSystemDirectoryState {
        lock.lock()
        defer { lock.unlock() }
        if entriesByDirectory[path] != nil {
            return .directory
        }
        let parent = (path as NSString).deletingLastPathComponent
        let name = (path as NSString).lastPathComponent
        guard let siblings = entriesByDirectory[parent] else { return .directory }
        guard siblings.contains(name) else { return .missing }
        return directoriesByParent[parent]?.contains(name) == true ? .directory : .notDirectory
    }
}

enum RemoteProjectPickerError: LocalizedError {
    case listingFailed

    var errorDescription: String? {
        switch self {
        case .listingFailed: "Failed to list remote directory."
        }
    }
}
