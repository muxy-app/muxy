import Foundation

struct FileTreeEntry: Hashable {
    let name: String
    let absolutePath: String
    let relativePath: String
    let isDirectory: Bool
    let isIgnored: Bool
}

enum FileTreeService {
    static func loadChildren(of directoryAbsolutePath: String, repoRoot: String) async throws -> [FileTreeEntry] {
        try await GitProcessRunner.offMainThrowing {
            try loadChildrenSync(of: directoryAbsolutePath, repoRoot: repoRoot)
        }
    }

    private static func loadChildrenSync(
        of directoryAbsolutePath: String,
        repoRoot: String
    ) throws -> [FileTreeEntry] {
        let fm = FileManager.default
        var isDirectory: ObjCBool = false
        guard fm.fileExists(atPath: directoryAbsolutePath, isDirectory: &isDirectory) else {
            throw FileSystemOperationError.sourceMissing(directoryAbsolutePath)
        }
        guard isDirectory.boolValue else {
            throw FileSystemOperationError.underlying(
                "“\((directoryAbsolutePath as NSString).lastPathComponent)” is not a directory"
            )
        }
        let contents: [String]
        do {
            contents = try fm.contentsOfDirectory(atPath: directoryAbsolutePath)
        } catch {
            throw FileSystemOperationError.underlying(error.localizedDescription)
        }

        let classification = classifyNames(in: directoryAbsolutePath, repoRoot: repoRoot, candidates: contents)
        let normalizedRoot = repoRoot.hasSuffix("/") ? String(repoRoot.dropLast()) : repoRoot

        var entries: [FileTreeEntry] = []
        entries.reserveCapacity(classification.visible.count)

        for name in classification.visible {
            if name == "." || name == ".." {
                continue
            }
            let absolute = directoryAbsolutePath.hasSuffix("/")
                ? directoryAbsolutePath + name
                : directoryAbsolutePath + "/" + name

            var isDir: ObjCBool = false
            guard fm.fileExists(atPath: absolute, isDirectory: &isDir) else { continue }

            let relative: String = if absolute.hasPrefix(normalizedRoot + "/") {
                String(absolute.dropFirst(normalizedRoot.count + 1))
            } else {
                name
            }

            entries.append(FileTreeEntry(
                name: name,
                absolutePath: absolute,
                relativePath: relative,
                isDirectory: isDir.boolValue,
                isIgnored: classification.ignored.contains(name)
            ))
        }

        entries.sort { lhs, rhs in
            if lhs.isDirectory != rhs.isDirectory {
                return lhs.isDirectory && !rhs.isDirectory
            }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }

        return entries
    }

    private struct NameClassification {
        let visible: [String]
        let ignored: Set<String>
    }

    private static func classifyNames(
        in directoryAbsolutePath: String,
        repoRoot: String,
        candidates: [String]
    ) -> NameClassification {
        let isRepoChild = isInsideRepo(path: directoryAbsolutePath, repoRoot: repoRoot)
        guard isRepoChild else {
            return NameClassification(visible: candidates, ignored: [])
        }

        let ignored = ignoredNames(directoryAbsolutePath: directoryAbsolutePath, candidates: candidates)
        let visible = candidates.filter { $0 != ".git" }
        return NameClassification(visible: visible, ignored: ignored)
    }

    private static func isInsideRepo(path: String, repoRoot: String) -> Bool {
        let normalizedRoot = repoRoot.hasSuffix("/") ? String(repoRoot.dropLast()) : repoRoot
        return path == normalizedRoot || path.hasPrefix(normalizedRoot + "/")
    }

    private static func ignoredNames(
        directoryAbsolutePath: String,
        candidates: [String]
    ) -> Set<String> {
        guard !candidates.isEmpty else { return [] }
        guard let gitPath = GitProcessRunner.resolveExecutable("git") else { return [] }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: gitPath)
        process.arguments = ["check-ignore", "-z", "--stdin"]
        process.currentDirectoryURL = URL(fileURLWithPath: directoryAbsolutePath)

        let stdinPipe = Pipe()
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardInput = stdinPipe
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        do {
            try process.run()
        } catch {
            return []
        }

        var payload = Data()
        for name in candidates {
            if let data = name.data(using: .utf8) {
                payload.append(data)
                payload.append(0)
            }
        }
        try? stdinPipe.fileHandleForWriting.write(contentsOf: payload)
        try? stdinPipe.fileHandleForWriting.close()

        let outData = (try? stdoutPipe.fileHandleForReading.readToEnd()) ?? Data()
        _ = try? stderrPipe.fileHandleForReading.readToEnd()
        process.waitUntilExit()

        var result: Set<String> = []
        var current = Data()
        for byte in outData {
            if byte == 0 {
                if let name = String(data: current, encoding: .utf8) {
                    result.insert(name)
                }
                current.removeAll(keepingCapacity: true)
            } else {
                current.append(byte)
            }
        }
        return result
    }
}
