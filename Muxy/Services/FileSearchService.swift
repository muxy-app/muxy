import Foundation

struct FileSearchResult: Identifiable {
    let id: String
    let relativePath: String
    let absolutePath: String
    let fileName: String
    let lowerFileName: String
    let lowerRelativePath: String
}

private struct GitignoreRule {
    let pattern: String
    let isNegated: Bool
    let isDirectoryOnly: Bool
    let isAnchored: Bool
}

actor FileSearchService {
    static let shared = FileSearchService()

    private var indexCache: [String: [FileSearchResult]] = [:]
    private var indexTasks: [String: Task<[FileSearchResult], Never>] = [:]

    private static let ignoredDirectories: Set<String> = [
        ".git", "node_modules", ".build", "build", "DerivedData", ".DS_Store",
        "__pycache__", ".tox", ".venv", "venv", ".env", "dist", ".next",
        ".nuxt", "target", "Pods", ".swiftpm", ".idea", ".vscode",
        "vendor", "coverage", ".cache", ".parcel-cache",
    ]

    private static let ignoredExtensions: Set<String> = [
        "png", "jpg", "jpeg", "gif", "bmp", "ico", "icns", "webp", "svg",
        "mp4", "mov", "avi", "mp3", "wav", "ogg",
        "zip", "tar", "gz", "rar", "7z",
        "pdf", "doc", "docx", "xls", "xlsx",
        "exe", "dll", "dylib", "so", "a", "o", "obj",
        "class", "jar", "pyc", "pyo",
        "woff", "woff2", "ttf", "otf", "eot",
        "sqlite", "db",
        "lock",
    ]

    func indexProject(_ projectPath: String) async -> [FileSearchResult] {
        if let cached = indexCache[projectPath] {
            return cached
        }

        if let existingTask = indexTasks[projectPath] {
            return await existingTask.value
        }

        let task = Task<[FileSearchResult], Never> {
            await buildIndex(projectPath: projectPath)
        }
        indexTasks[projectPath] = task
        let results = await task.value
        indexCache[projectPath] = results
        indexTasks.removeValue(forKey: projectPath)
        return results
    }

    func getIndex(projectPath: String) async -> [FileSearchResult] {
        await indexProject(projectPath)
    }

    static func search(query: String, in index: [FileSearchResult]) -> [FileSearchResult] {
        guard !query.isEmpty else { return Array(index.prefix(200)) }

        let lowerQuery = query.lowercased()
        var scored: [(result: FileSearchResult, score: Int)] = []

        for file in index {
            if file.lowerFileName == lowerQuery {
                scored.append((file, 1000))
            } else if file.lowerFileName.hasPrefix(lowerQuery) {
                scored.append((file, 800))
            } else if file.lowerFileName.contains(lowerQuery) {
                scored.append((file, 600))
            } else if fuzzyMatch(query: lowerQuery, target: file.lowerFileName) {
                scored.append((file, 400))
            } else if file.lowerRelativePath.contains(lowerQuery) {
                scored.append((file, 200))
            } else if fuzzyMatch(query: lowerQuery, target: file.lowerRelativePath) {
                scored.append((file, 100))
            }
        }

        scored.sort { lhs, rhs in
            if lhs.score != rhs.score { return lhs.score > rhs.score }
            return lhs.result.relativePath.count < rhs.result.relativePath.count
        }

        return scored.prefix(200).map(\.result)
    }

    func invalidateCache(projectPath: String) {
        indexCache.removeValue(forKey: projectPath)
    }

    private func buildIndex(projectPath: String) async -> [FileSearchResult] {
        let baseURL = URL(fileURLWithPath: projectPath)
        let fileManager = FileManager.default
        var results: [FileSearchResult] = []

        guard let enumerator = fileManager.enumerator(
            at: baseURL,
            includingPropertiesForKeys: [.isDirectoryKey, .isRegularFileKey],
            options: []
        )
        else { return [] }

        let gitignoreRules = loadGitignore(at: projectPath)

        while let url = enumerator.nextObject() as? URL {
            if Task.isCancelled { break }

            let name = url.lastPathComponent

            if Self.ignoredDirectories.contains(name) {
                enumerator.skipDescendants()
                continue
            }

            let resourceValues = try? url.resourceValues(forKeys: [.isRegularFileKey])
            guard resourceValues?.isRegularFile == true else { continue }

            let ext = url.pathExtension.lowercased()
            guard !Self.ignoredExtensions.contains(ext) else { continue }

            let relativePath = String(url.path(percentEncoded: false).dropFirst(projectPath.count + 1))

            guard !matchesGitignore(relativePath, rules: gitignoreRules) else { continue }

            let absolutePath = url.path(percentEncoded: false)
            results.append(FileSearchResult(
                id: absolutePath,
                relativePath: relativePath,
                absolutePath: absolutePath,
                fileName: name,
                lowerFileName: name.lowercased(),
                lowerRelativePath: relativePath.lowercased()
            ))
        }

        results.sort { $0.relativePath.localizedStandardCompare($1.relativePath) == .orderedAscending }
        return results
    }

    private static func fuzzyMatch(query: String, target: String) -> Bool {
        var queryIterator = query.utf8.makeIterator()
        guard var queryByte = queryIterator.next() else { return true }
        for targetByte in target.utf8 where targetByte == queryByte {
            guard let next = queryIterator.next() else { return true }
            queryByte = next
        }
        return false
    }

    private func loadGitignore(at projectPath: String) -> [GitignoreRule] {
        let gitignorePath = projectPath + "/.gitignore"
        guard let content = try? String(contentsOfFile: gitignorePath, encoding: .utf8) else {
            return []
        }
        return content
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .compactMap(parseGitignoreRule)
    }

    private func parseGitignoreRule(_ line: String) -> GitignoreRule? {
        guard !line.isEmpty, !line.hasPrefix("#") else { return nil }
        let isNegated = line.hasPrefix("!")
        let rawPattern = isNegated ? String(line.dropFirst()) : line
        guard !rawPattern.isEmpty else { return nil }
        let isDirectoryOnly = rawPattern.hasSuffix("/")
        let isAnchored = rawPattern.hasPrefix("/")
        let pattern = rawPattern
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            .replacingOccurrences(of: "**", with: "*")
        guard !pattern.isEmpty else { return nil }
        return GitignoreRule(
            pattern: pattern,
            isNegated: isNegated,
            isDirectoryOnly: isDirectoryOnly,
            isAnchored: isAnchored
        )
    }

    private func matchesGitignore(_ path: String, rules: [GitignoreRule]) -> Bool {
        let normalizedPath = path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let pathParts = normalizedPath.split(separator: "/").map(String.init)
        let directoryPrefixes = directoryPrefixPaths(pathParts: pathParts)

        var isIgnored = false
        for rule in rules {
            if rule.isDirectoryOnly {
                if directoryPrefixes.contains(where: { matchesRule(rule, candidate: $0) }) {
                    isIgnored = !rule.isNegated
                }
                continue
            }

            if matchesRule(rule, candidate: normalizedPath) {
                isIgnored = !rule.isNegated
            }
        }
        return isIgnored
    }

    private func matchesRule(_ rule: GitignoreRule, candidate: String) -> Bool {
        if wildcardMatch(pattern: rule.pattern, text: candidate) {
            return true
        }
        guard !rule.isAnchored else { return false }
        let parts = candidate.split(separator: "/").map(String.init)
        guard parts.count > 1 else { return false }
        for index in 1 ..< parts.count {
            let suffix = parts[index...].joined(separator: "/")
            if wildcardMatch(pattern: rule.pattern, text: suffix) {
                return true
            }
        }
        return false
    }

    private func wildcardMatch(pattern: String, text: String) -> Bool {
        pattern.withCString { patternCString in
            text.withCString { textCString in
                fnmatch(patternCString, textCString, 0) == 0
            }
        }
    }

    private func directoryPrefixPaths(pathParts: [String]) -> [String] {
        guard pathParts.count > 1 else { return [] }
        var prefixes: [String] = []
        for index in 0 ..< (pathParts.count - 1) {
            prefixes.append(pathParts[0 ... index].joined(separator: "/"))
        }
        return prefixes
    }
}
