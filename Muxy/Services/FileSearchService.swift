import Foundation

struct FileSearchResult: Identifiable {
    let id: String
    let relativePath: String
    let absolutePath: String
    let fileName: String
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

    func search(query: String, projectPath: String) async -> [FileSearchResult] {
        let index = await indexProject(projectPath)
        guard !query.isEmpty else { return Array(index.prefix(200)) }

        let lowerQuery = query.lowercased()
        var scored: [(result: FileSearchResult, score: Int)] = []

        for file in index {
            let lowerPath = file.relativePath.lowercased()
            let lowerName = file.fileName.lowercased()

            if lowerName == lowerQuery {
                scored.append((file, 1000))
            } else if lowerName.hasPrefix(lowerQuery) {
                scored.append((file, 800))
            } else if lowerName.contains(lowerQuery) {
                scored.append((file, 600))
            } else if fuzzyMatch(query: lowerQuery, target: lowerName) {
                scored.append((file, 400))
            } else if lowerPath.contains(lowerQuery) {
                scored.append((file, 200))
            } else if fuzzyMatch(query: lowerQuery, target: lowerPath) {
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
            options: [.skipsHiddenFiles]
        )
        else { return [] }

        let gitignorePatterns = loadGitignore(at: projectPath)

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

            guard !matchesGitignore(relativePath, patterns: gitignorePatterns) else { continue }

            results.append(FileSearchResult(
                id: url.path(percentEncoded: false),
                relativePath: relativePath,
                absolutePath: url.path(percentEncoded: false),
                fileName: name
            ))
        }

        results.sort { $0.relativePath.localizedStandardCompare($1.relativePath) == .orderedAscending }
        return results
    }

    private func fuzzyMatch(query: String, target: String) -> Bool {
        var queryIndex = query.startIndex
        var targetIndex = target.startIndex

        while queryIndex < query.endIndex, targetIndex < target.endIndex {
            if query[queryIndex] == target[targetIndex] {
                queryIndex = query.index(after: queryIndex)
            }
            targetIndex = target.index(after: targetIndex)
        }

        return queryIndex == query.endIndex
    }

    private func loadGitignore(at projectPath: String) -> [String] {
        let gitignorePath = projectPath + "/.gitignore"
        guard let content = try? String(contentsOfFile: gitignorePath, encoding: .utf8) else {
            return []
        }
        return content
            .components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty && !$0.hasPrefix("#") }
    }

    private func matchesGitignore(_ path: String, patterns: [String]) -> Bool {
        for pattern in patterns {
            let cleanPattern = pattern.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
            if path.contains(cleanPattern) { return true }
        }
        return false
    }
}
