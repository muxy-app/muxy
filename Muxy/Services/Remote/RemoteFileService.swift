import Foundation
import MuxyShared

extension WorkspaceContext {
    var remoteFileService: RemoteFileService? {
        guard case let .ssh(destination) = self else { return nil }
        return RemoteFileService(destination: destination)
    }
}

struct RemoteFileService {
    typealias Runner = @Sendable (SSHDestination, String, Data?) async throws -> GitProcessResult

    let destination: SSHDestination
    private let runner: Runner

    init(
        destination: SSHDestination,
        runner: @escaping Runner = { destination, command, input in
            try await SSHCommandRunner.run(
                destination: destination,
                remoteCommand: command,
                input: input
            )
        }
    ) {
        self.destination = destination
        self.runner = runner
    }

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

    func read(
        root: String,
        relativePath: String,
        maxBytes: Int,
        encoding: FileEncodingDTO
    ) async throws -> WorkspaceFileService.ReadResult {
        let absolute = try contained(root: root, relativePath: relativePath)
        let quoted = RemoteCommandBuilder.quoteRemotePath(absolute)
        let result = try await runGuarded(root: root, targets: [absolute], "head -c \(maxBytes + 1) \(quoted)")
        guard result.status == 0 else {
            throw FileSystemOperationError.sourceMissing(absolute)
        }
        let size = result.stdoutData.count
        guard size <= maxBytes else {
            throw FileSystemOperationError.underlying("file exceeds \(maxBytes) byte read limit")
        }
        return try WorkspaceFileService.ReadResult(
            relativePath: relative(absolute, root: root),
            content: WorkspaceFileService.encode(result.stdoutData, as: encoding),
            size: size,
            encoding: encoding
        )
    }

    func stat(root: String, relativePath: String) async throws -> WorkspaceFileService.StatResult {
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
        return WorkspaceFileService.StatResult(
            name: (absolute as NSString).lastPathComponent,
            relativePath: relative(absolute, root: root),
            isDirectory: isDirectory,
            size: size
        )
    }

    func write(
        root: String,
        relativePath: String,
        data: Data,
        maxBytes: Int
    ) async throws -> String {
        guard data.count <= maxBytes else {
            throw FileSystemOperationError.underlying("file exceeds \(maxBytes) byte write limit")
        }
        let absolute = try containedMutation(root: root, relativePath: relativePath)
        let quoted = RemoteCommandBuilder.quoteRemotePath(absolute)
        let command = "__muxy_write_target=$(__muxy_resolve \(quoted)) "
            + "|| exit \(RemoteCommandBuilder.containmentEscapeExitCode); "
            + "__muxy_write_target=${__muxy_write_target%?}; "
            + "__muxy_require_contained \"$__muxy_write_target\"; "
            + "[ ! -d \"$__muxy_write_target\" ] || exit 7; "
            + "__muxy_write_parent=${__muxy_write_target%/*}; "
            + "[ -n \"$__muxy_write_parent\" ] || __muxy_write_parent=/; "
            + "__muxy_write_mode=; "
            + "if [ -e \"$__muxy_write_target\" ]; then "
            + "__muxy_write_mode=$(stat -f '%Lp' \"$__muxy_write_target\" 2>/dev/null) "
            + "|| __muxy_write_mode=; "
            + "case \"$__muxy_write_mode\" in ''|*[!0-7]*) "
            + "__muxy_write_mode=$(stat -c '%a' \"$__muxy_write_target\" 2>/dev/null) || exit 1 ;; "
            + "esac; "
            + "case \"$__muxy_write_mode\" in ''|*[!0-7]*) exit 1 ;; esac; "
            + "fi; "
            + "__muxy_write_temp_dir=$(mktemp -d \"$__muxy_write_parent/.muxy-write.XXXXXX\") || exit 1; "
            + "__muxy_write_temp=\"$__muxy_write_temp_dir/content\"; "
            + "if cat > \"$__muxy_write_temp\" "
            + "&& [ \"$(wc -c < \"$__muxy_write_temp\")\" -eq \(data.count) ] "
            + "&& { [ -z \"$__muxy_write_mode\" ] "
            + "|| chmod \"$__muxy_write_mode\" \"$__muxy_write_temp\"; } "
            + "&& mv -f \"$__muxy_write_temp\" \"$__muxy_write_target\"; then :; "
            + "else __muxy_write_status=$?; rm -f \"$__muxy_write_temp\"; "
            + "rmdir \"$__muxy_write_temp_dir\" 2>/dev/null || true; "
            + "exit \"$__muxy_write_status\"; fi; "
            + "rmdir \"$__muxy_write_temp_dir\" 2>/dev/null || true"
        let result = try await runGuarded(root: root, targets: [absolute], command, input: data)
        if result.status == 7 {
            throw FileSystemOperationError.underlying(
                "“\((absolute as NSString).lastPathComponent)” is a directory"
            )
        }
        guard result.status == 0 else {
            throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "write failed" : result.stderr)
        }
        return relative(absolute, root: root)
    }

    func mkdir(root: String, relativePath: String) async throws -> String {
        let absolute = try containedMutation(root: root, relativePath: relativePath)
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
        let name = try FileSystemOperations.sanitize(newName)
        let absolute = try containedMutation(root: root, relativePath: relativePath)
        let parent = (absolute as NSString).deletingLastPathComponent
        let target = try containedAbsoluteMutation(
            root: root,
            absolutePath: (parent as NSString).appendingPathComponent(name)
        )
        let quotedSource = RemoteCommandBuilder.quoteRemotePath(absolute)
        if target == absolute {
            let result = try await runGuarded(
                root: root,
                targets: [absolute],
                "{ [ -e \(quotedSource) ] || [ -L \(quotedSource) ]; }"
            )
            guard result.status == 0 else {
                throw FileSystemOperationError.sourceMissing(absolute)
            }
            return relative(absolute, root: root)
        }
        let quotedTarget = RemoteCommandBuilder.quoteRemotePath(target)
        let script = "if { [ ! -e \(quotedSource) ] && [ ! -L \(quotedSource) ]; }; then exit 6; fi; "
            + "if { [ -e \(quotedTarget) ] || [ -L \(quotedTarget) ]; }; then exit 8; fi; "
            + "__muxy_require_contained \(quotedTarget); "
            + "mv \(quotedSource) \(quotedTarget)"
        let result = try await runGuarded(
            root: root,
            targets: [absolute],
            script
        )
        if result.status == 6 {
            throw FileSystemOperationError.sourceMissing(absolute)
        }
        if result.status == 8 {
            throw FileSystemOperationError.destinationExists(target)
        }
        guard result.status == 0 else {
            throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "rename failed" : result.stderr)
        }
        return relative(target, root: root)
    }

    func move(root: String, paths: [String], into destinationRelative: String) async throws -> [String] {
        let destination = try contained(root: root, relativePath: destinationRelative)
        var moved: [String] = []
        for path in paths {
            let source = try containedMutation(root: root, relativePath: path)
            let sourceParent = (source as NSString).deletingLastPathComponent
            if sourceParent == destination {
                let quotedSource = RemoteCommandBuilder.quoteRemotePath(source)
                let result = try await runGuarded(
                    root: root,
                    targets: [source, destination],
                    "{ [ -e \(quotedSource) ] || [ -L \(quotedSource) ]; }"
                )
                guard result.status == 0 else {
                    throw FileSystemOperationError.sourceMissing(source)
                }
                moved.append(relative(source, root: root))
                continue
            }
            if destination == source || destination.hasPrefix(source + "/") {
                throw FileSystemOperationError.sameAsSource
            }
            let name = (source as NSString).lastPathComponent
            let script = uniqueMoveScript(source: source, destination: destination, name: name)
            let result = try await runGuarded(
                root: root,
                targets: [source, destination],
                script
            )
            if result.status == 6 {
                throw FileSystemOperationError.sourceMissing(source)
            }
            guard result.status == 0 else {
                throw FileSystemOperationError.underlying(result.stderr.isEmpty ? "move failed" : result.stderr)
            }
            guard let actualTarget = String(data: result.stdoutData, encoding: .utf8),
                  !actualTarget.isEmpty
            else {
                throw FileSystemOperationError.underlying("move failed")
            }
            let validatedTarget = try containedAbsoluteMutation(root: root, absolutePath: actualTarget)
            moved.append(relative(validatedTarget, root: root))
        }
        return moved
    }

    func delete(root: String, paths: [String]) async throws {
        for path in paths {
            let absolute = try containedMutation(root: root, relativePath: path)
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
            throw FileSystemOperationError.outsideRoot(relativePath)
        }
        return resolved
    }

    private func relative(_ absolute: String, root: String) -> String {
        let normalizedRoot = ProjectPickerPathService.standardizedRemotePath(root)
        let normalized = ProjectPickerPathService.standardizedRemotePath(absolute)
        guard normalized != normalizedRoot else { return "" }
        guard normalized.hasPrefix(normalizedRoot + "/") else { return (absolute as NSString).lastPathComponent }
        return String(normalized.dropFirst(normalizedRoot.count + 1))
    }

    private func containedMutation(root: String, relativePath: String) throws -> String {
        let absolute = try contained(root: root, relativePath: relativePath)
        guard absolute != ProjectPickerPathService.standardizedRemotePath(root) else {
            throw FileSystemOperationError.outsideRoot(relativePath)
        }
        return absolute
    }

    private func containedAbsoluteMutation(root: String, absolutePath: String) throws -> String {
        let normalizedRoot = ProjectPickerPathService.standardizedRemotePath(root)
        let normalized = ProjectPickerPathService.standardizedRemotePath(absolutePath)
        guard normalized.hasPrefix(normalizedRoot + "/") else {
            throw FileSystemOperationError.outsideRoot(absolutePath)
        }
        return normalized
    }

    private func uniqueMoveScript(source: String, destination: String, name: String) -> String {
        let pathExtension = (name as NSString).pathExtension
        let stem = (name as NSString).deletingPathExtension
        let quotedSource = RemoteCommandBuilder.quoteRemotePath(source)
        let quotedDestination = RemoteCommandBuilder.quoteRemotePath(destination)
        let quotedName = ShellEscaper.escape(name)
        let quotedStem = ShellEscaper.escape(stem)
        let quotedExtension = ShellEscaper.escape(pathExtension)
        return "if { [ ! -e \(quotedSource) ] && [ ! -L \(quotedSource) ]; }; then exit 6; fi; "
            + "[ -d \(quotedDestination) ] || exit 7; "
            + "__muxy_destination=\(quotedDestination); __muxy_name=\(quotedName); "
            + "__muxy_stem=\(quotedStem); __muxy_extension=\(quotedExtension); __muxy_counter=2; "
            + "__muxy_target=\"$__muxy_destination/$__muxy_name\"; "
            + "while [ -e \"$__muxy_target\" ] || [ -L \"$__muxy_target\" ]; do "
            + "if [ -n \"$__muxy_extension\" ]; then "
            + "__muxy_name=\"$__muxy_stem $__muxy_counter.$__muxy_extension\"; "
            + "else __muxy_name=\"$__muxy_stem $__muxy_counter\"; fi; "
            + "__muxy_target=\"$__muxy_destination/$__muxy_name\"; "
            + "__muxy_counter=$((__muxy_counter + 1)); "
            + "done; "
            + "__muxy_require_contained \"$__muxy_target\"; "
            + "mv \(quotedSource) \"$__muxy_target\" && printf '%s' \"$__muxy_target\""
    }

    private func run(_ remoteCommand: String, input: Data? = nil) async throws -> GitProcessResult {
        try await runner(destination, remoteCommand, input)
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
            throw FileSystemOperationError.outsideRoot("")
        }
        return result
    }
}
