import Foundation

extension WorkspaceContext {
    var remoteFileService: RemoteFileService? {
        guard case let .ssh(destination) = self else { return nil }
        return RemoteFileService(destination: destination)
    }
}

struct RemoteFileService {
    let destination: SSHDestination

    func list(root: String, relativePath: String) async throws -> [FileTreeEntry] {
        let directory = try contained(root: root, relativePath: relativePath)
        let quoted = RemoteCommandBuilder.quoteRemotePath(directory)
        let script = "cd \(quoted) && for e in * .*; do "
            + "case \"$e\" in '.'|'..'|'*'|'.*') continue ;; esac; "
            + "{ [ -e \"$e\" ] || [ -L \"$e\" ]; } || continue; "
            + "if [ -d \"$e\" ]; then printf 'd %s\\0' \"$e\"; "
            + "else printf 'f %s\\0' \"$e\"; fi; done"
        let result = try await runGuarded(root: root, targets: [directory], script)
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            throw FileSystemOperationError.underlying(detail.isEmpty ? "could not list '\(directory)'" : detail)
        }
        return parseEntries(result.stdout, directory: directory, root: root)
    }

    func read(root: String, relativePath: String, maxBytes: Int) async throws -> MuxyAPI.Files.ReadResult {
        let absolute = try contained(root: root, relativePath: relativePath)
        let quoted = RemoteCommandBuilder.quoteRemotePath(absolute)
        let sizeResult = try await runGuarded(root: root, targets: [absolute], "wc -c < \(quoted)")
        let size = Int(sizeResult.stdout.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
        guard size <= maxBytes else {
            throw FileSystemOperationError.underlying("file exceeds \(maxBytes) byte read limit")
        }
        let result = try await runGuarded(root: root, targets: [absolute], "cat \(quoted)")
        guard result.status == 0 else {
            throw FileSystemOperationError.sourceMissing(absolute)
        }
        return MuxyAPI.Files.ReadResult(
            relativePath: relative(absolute, root: root),
            content: result.stdout,
            size: size
        )
    }

    func stat(root: String, relativePath: String) async throws -> MuxyAPI.Files.StatResult {
        let absolute = try contained(root: root, relativePath: relativePath)
        let quoted = RemoteCommandBuilder.quoteRemotePath(absolute)
        let script = "if [ -d \(quoted) ]; then printf 'd '; elif [ -e \(quoted) ]; then printf 'f '; "
            + "else exit 7; fi; wc -c < \(quoted) 2>/dev/null || echo 0"
        let result = try await runGuarded(root: root, targets: [absolute], script)
        guard result.status == 0 else {
            throw FileSystemOperationError.sourceMissing(absolute)
        }
        let output = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        let isDirectory = output.hasPrefix("d")
        let size = Int(output.dropFirst(2).trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
        return MuxyAPI.Files.StatResult(
            name: (absolute as NSString).lastPathComponent,
            relativePath: relative(absolute, root: root),
            isDirectory: isDirectory,
            size: size
        )
    }

    func write(root: String, relativePath: String, contents: String) async throws -> String {
        let absolute = try contained(root: root, relativePath: relativePath)
        let quoted = RemoteCommandBuilder.quoteRemotePath(absolute)
        let command = "base64 -d > \(quoted)"
        let encoded = Data(Data(contents.utf8).base64EncodedString().utf8)
        let result = try await runGuarded(root: root, targets: [absolute], command, input: encoded)
        guard result.status == 0 else {
            throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "write failed" : result.stderr)
        }
        return relative(absolute, root: root)
    }

    func mkdir(root: String, relativePath: String) async throws -> String {
        let absolute = try contained(root: root, relativePath: relativePath)
        let result = try await runGuarded(
            root: root,
            targets: [absolute],
            "mkdir -p \(RemoteCommandBuilder.quoteRemotePath(absolute))"
        )
        guard result.status == 0 else {
            throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "mkdir failed" : result.stderr)
        }
        return relative(absolute, root: root)
    }

    func rename(root: String, relativePath: String, newName: String) async throws -> String {
        let absolute = try contained(root: root, relativePath: relativePath)
        let parent = (absolute as NSString).deletingLastPathComponent
        let target = (parent as NSString).appendingPathComponent(newName)
        try requireSimpleName(newName)
        let result = try await runGuarded(
            root: root,
            targets: [absolute, target],
            "mv \(RemoteCommandBuilder.quoteRemotePath(absolute)) \(RemoteCommandBuilder.quoteRemotePath(target))"
        )
        guard result.status == 0 else {
            throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "rename failed" : result.stderr)
        }
        return relative(target, root: root)
    }

    func move(root: String, paths: [String], into destinationRelative: String) async throws -> [String] {
        let destination = try contained(root: root, relativePath: destinationRelative)
        var moved: [String] = []
        for path in paths {
            let source = try contained(root: root, relativePath: path)
            let target = (destination as NSString).appendingPathComponent((source as NSString).lastPathComponent)
            let result = try await runGuarded(
                root: root,
                targets: [source, target],
                "mv \(RemoteCommandBuilder.quoteRemotePath(source)) \(RemoteCommandBuilder.quoteRemotePath(target))"
            )
            guard result.status == 0 else {
                throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "move failed" : result.stderr)
            }
            moved.append(relative(target, root: root))
        }
        return moved
    }

    func delete(root: String, paths: [String]) async throws {
        for path in paths {
            let absolute = try contained(root: root, relativePath: path)
            let result = try await runGuarded(
                root: root,
                targets: [absolute],
                "rm -rf \(RemoteCommandBuilder.quoteRemotePath(absolute))"
            )
            guard result.status == 0 else {
                throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "delete failed" : result.stderr)
            }
        }
    }

    private func parseEntries(_ output: String, directory: String, root: String) -> [FileTreeEntry] {
        output
            .split(separator: "\0", omittingEmptySubsequences: true)
            .compactMap { record -> FileTreeEntry? in
                guard record.count > 2 else { return nil }
                let isDirectory = record.first == "d"
                let name = String(record.dropFirst(2))
                guard !name.isEmpty, name != ".git" else { return nil }
                let absolute = directory.hasSuffix("/") ? directory + name : directory + "/" + name
                return FileTreeEntry(
                    name: name,
                    absolutePath: absolute,
                    relativePath: relative(absolute, root: root),
                    isDirectory: isDirectory,
                    isIgnored: false
                )
            }
            .sorted { lhs, rhs in
                if lhs.isDirectory != rhs.isDirectory {
                    return lhs.isDirectory && !rhs.isDirectory
                }
                return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
            }
    }

    private func contained(root: String, relativePath: String) throws -> String {
        let normalizedRoot = ProjectPickerPathService.standardizedRemotePath(root)
        let trimmed = relativePath.hasPrefix("/") ? String(relativePath.dropFirst()) : relativePath
        let joined = trimmed.isEmpty ? normalizedRoot : normalizedRoot + "/" + trimmed
        let resolved = ProjectPickerPathService.standardizedRemotePath(joined)
        guard resolved == normalizedRoot || resolved.hasPrefix(normalizedRoot + "/") else {
            throw FileSystemOperationError.underlying("path '\(relativePath)' escapes the workspace root")
        }
        return resolved
    }

    private func relative(_ absolute: String, root: String) -> String {
        let normalizedRoot = ProjectPickerPathService.standardizedRemotePath(root)
        let normalized = ProjectPickerPathService.standardizedRemotePath(absolute)
        guard normalized.hasPrefix(normalizedRoot + "/") else { return (absolute as NSString).lastPathComponent }
        return String(normalized.dropFirst(normalizedRoot.count + 1))
    }

    private func requireSimpleName(_ name: String) throws {
        guard !name.contains("/"), name != ".", name != ".." else {
            throw FileSystemOperationError.underlying("invalid name '\(name)'")
        }
    }

    private func run(_ remoteCommand: String, input: Data? = nil) async throws -> GitProcessResult {
        try await SSHCommandRunner.run(destination: destination, remoteCommand: remoteCommand, input: input)
    }

    private func runGuarded(
        root: String,
        targets: [String],
        _ remoteCommand: String,
        input: Data? = nil
    ) async throws -> GitProcessResult {
        let normalizedRoot = ProjectPickerPathService.standardizedRemotePath(root)
        let guards = targets
            .map { RemoteCommandBuilder.containmentGuardPrefix(root: normalizedRoot, target: $0) }
            .joined()
        let result = try await run(guards + remoteCommand, input: input)
        guard result.status != RemoteCommandBuilder.containmentEscapeExitCode else {
            throw FileSystemOperationError.underlying("path escapes the workspace root")
        }
        return result
    }
}
