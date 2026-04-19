import Foundation

struct FileTreeEntry: Hashable {
    let name: String
    let absolutePath: String
    let relativePath: String
    let isDirectory: Bool
}

enum FileTreeService {
    private static let prunedDirectoryNames: Set<String> = [
        ".git", "node_modules", ".build", "build", "DerivedData",
        "__pycache__", ".venv", "venv", "dist", ".next", ".nuxt",
        "target", "Pods", ".swiftpm", ".idea", ".vscode",
        "vendor", "coverage", ".cache", ".parcel-cache",
    ]

    static func loadChildren(of directoryAbsolutePath: String, repoRoot: String) async -> [FileTreeEntry] {
        await GitProcessRunner.offMain {
            loadChildrenSync(of: directoryAbsolutePath, repoRoot: repoRoot)
        }
    }

    private static func loadChildrenSync(of directoryAbsolutePath: String, repoRoot: String) -> [FileTreeEntry] {
        let fm = FileManager.default
        guard let contents = try? fm.contentsOfDirectory(atPath: directoryAbsolutePath) else {
            return []
        }

        let allowed = allowedNames(in: directoryAbsolutePath, repoRoot: repoRoot, candidates: contents)
        let normalizedRoot = repoRoot.hasSuffix("/") ? String(repoRoot.dropLast()) : repoRoot

        var entries: [FileTreeEntry] = []
        entries.reserveCapacity(allowed.count)

        for name in allowed {
            if name == "." || name == ".." { continue }
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
                isDirectory: isDir.boolValue
            ))
        }

        entries.sort { lhs, rhs in
            if lhs.isDirectory != rhs.isDirectory { return lhs.isDirectory && !rhs.isDirectory }
            return lhs.name.localizedCaseInsensitiveCompare(rhs.name) == .orderedAscending
        }

        return entries
    }

    private static func allowedNames(
        in directoryAbsolutePath: String,
        repoRoot: String,
        candidates: [String]
    ) -> [String] {
        let isRepoChild = isInsideRepo(path: directoryAbsolutePath, repoRoot: repoRoot)
        guard isRepoChild else {
            return candidates.filter { !prunedDirectoryNames.contains($0) }
        }

        let ignored = ignoredNames(directoryAbsolutePath: directoryAbsolutePath, candidates: candidates)
        return candidates.filter { name in
            if name == ".git" { return false }
            return !ignored.contains(name)
        }
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
        process.arguments = ["check-ignore", "--stdin"]
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

        let payload = candidates.joined(separator: "\n") + "\n"
        if let data = payload.data(using: .utf8) {
            stdinPipe.fileHandleForWriting.write(data)
        }
        try? stdinPipe.fileHandleForWriting.close()

        let outData = (try? stdoutPipe.fileHandleForReading.readToEnd()) ?? Data()
        _ = try? stderrPipe.fileHandleForReading.readToEnd()
        process.waitUntilExit()

        guard let output = String(data: outData, encoding: .utf8) else { return [] }
        var result: Set<String> = []
        for line in output.split(separator: "\n", omittingEmptySubsequences: true) {
            result.insert(String(line))
        }
        return result
    }
}
