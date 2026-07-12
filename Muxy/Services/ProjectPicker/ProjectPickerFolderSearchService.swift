import Foundation
import os

private let projectPickerFolderSearchLogger = Logger(
    subsystem: "app.muxy",
    category: "ProjectPickerFolderSearch"
)

struct ProjectPickerFolderSearchResult: Equatable, Hashable, Identifiable {
    let name: String
    let path: String
    let displayPath: String

    var id: String { path }
}

enum ProjectPickerFolderSearchFailure: Equatable {
    case invalidRoot
    case unreadableRoot
    case scanFailed
}

struct ProjectPickerFolderSearchSnapshot: Equatable {
    let results: [ProjectPickerFolderSearchResult]
    let readFailed: Bool
    let failure: ProjectPickerFolderSearchFailure?
    let isTruncated: Bool
    let hasMoreResults: Bool

    init(
        results: [ProjectPickerFolderSearchResult],
        readFailed: Bool,
        failure: ProjectPickerFolderSearchFailure? = nil,
        isTruncated: Bool = false,
        hasMoreResults: Bool = false
    ) {
        self.results = results
        self.readFailed = readFailed
        self.failure = failure ?? (readFailed ? .scanFailed : nil)
        self.isTruncated = isTruncated
        self.hasMoreResults = hasMoreResults
    }
}

enum ProjectPickerFolderSearchRootState {
    case ready
    case invalid
    case unreadable
}

enum ProjectPickerFolderSearchItemState {
    case directory
    case file
    case missing
}

struct ProjectPickerFolderSearchIndexEntry: Equatable {
    let name: String
    let path: String
    let foldedName: String
    let foldedSearchPath: String
}

struct ProjectPickerFolderSearchIndex: Equatable {
    let entries: [ProjectPickerFolderSearchIndexEntry]
    let isTruncated: Bool
}

protocol ProjectPickerFolderSearchFileSystem: Sendable {
    func rootState(atPath path: String) -> ProjectPickerFolderSearchRootState
    func itemState(atPath path: String) -> ProjectPickerFolderSearchItemState
    func scanDirectories(
        atPath path: String,
        maximumDirectoryCount: Int,
        maximumVisitedEntryCount: Int,
        maximumDuration: Duration
    ) throws -> ProjectPickerFolderSearchIndex
}

protocol ProjectPickerFolderSearchWatching: AnyObject, Sendable {}

extension FileSystemWatcher: ProjectPickerFolderSearchWatching {}

typealias ProjectPickerFolderSearchWatcherFactory = @Sendable (
    _ rootPath: String,
    _ handler: @escaping @Sendable ([String]) -> Void
) -> (any ProjectPickerFolderSearchWatching)?

struct FileManagerProjectPickerFolderSearchFileSystem: ProjectPickerFolderSearchFileSystem {
    private static let skippedNames = Set([
        ".build",
        ".cache",
        ".gradle",
        ".next",
        "build",
        "carthage",
        "deriveddata",
        "dist",
        "node_modules",
        "pods",
        "target",
        "vendor",
    ])

    private var fileManager: FileManager { .default }

    func rootState(atPath path: String) -> ProjectPickerFolderSearchRootState {
        var isDirectory = ObjCBool(false)
        guard fileManager.fileExists(atPath: path, isDirectory: &isDirectory), isDirectory.boolValue else {
            return .invalid
        }
        return fileManager.isReadableFile(atPath: path) ? .ready : .unreadable
    }

    func itemState(atPath path: String) -> ProjectPickerFolderSearchItemState {
        var isDirectory = ObjCBool(false)
        guard fileManager.fileExists(atPath: path, isDirectory: &isDirectory) else { return .missing }
        return isDirectory.boolValue ? .directory : .file
    }

    func scanDirectories(
        atPath path: String,
        maximumDirectoryCount: Int,
        maximumVisitedEntryCount: Int,
        maximumDuration: Duration
    ) throws -> ProjectPickerFolderSearchIndex {
        try Task.checkCancellation()
        let scanStart = ContinuousClock.now
        let rootURL = URL(fileURLWithPath: path, isDirectory: true).standardizedFileURL
        let resourceKeys: [URLResourceKey] = [
            .isDirectoryKey,
            .isHiddenKey,
            .isPackageKey,
            .isSymbolicLinkKey,
            .nameKey,
        ]
        let resourceKeySet = Set(resourceKeys)
        guard let enumerator = fileManager.enumerator(
            at: rootURL,
            includingPropertiesForKeys: resourceKeys,
            options: [],
            errorHandler: { url, error in
                projectPickerFolderSearchLogger.debug(
                    "Skipping unreadable folder at \(url.path, privacy: .private): \(error.localizedDescription, privacy: .public)"
                )
                return true
            }
        )
        else {
            throw ProjectPickerFolderSearchFileSystemError.cannotEnumerate
        }

        var entries: [ProjectPickerFolderSearchIndexEntry] = []
        entries.reserveCapacity(min(maximumDirectoryCount, 4096))
        var isTruncated = false
        var visitedEntryCount = 0
        append(rootURL, rootURL: rootURL, to: &entries)

        for case let url as URL in enumerator {
            try Task.checkCancellation()
            visitedEntryCount += 1
            guard visitedEntryCount <= maximumVisitedEntryCount,
                  ContinuousClock.now - scanStart < maximumDuration
            else {
                isTruncated = true
                break
            }
            let values: URLResourceValues
            do {
                values = try url.resourceValues(forKeys: resourceKeySet)
            } catch {
                enumerator.skipDescendants()
                continue
            }
            guard values.isDirectory == true else { continue }
            guard entries.count < maximumDirectoryCount else {
                isTruncated = true
                break
            }

            let standardizedURL = url.standardizedFileURL
            append(standardizedURL, rootURL: rootURL, name: values.name, to: &entries)
            if shouldSkipDescendants(of: standardizedURL, values: values, rootURL: rootURL) {
                enumerator.skipDescendants()
            }
        }

        return ProjectPickerFolderSearchIndex(entries: entries, isTruncated: isTruncated)
    }

    private func append(
        _ url: URL,
        rootURL: URL,
        name suppliedName: String? = nil,
        to entries: inout [ProjectPickerFolderSearchIndexEntry]
    ) {
        let path = url.path
        let name = suppliedName.flatMap { $0.isEmpty ? nil : $0 } ?? ProjectPickerFolderSearchPath.name(for: path)
        entries.append(ProjectPickerFolderSearchIndexEntry(
            name: name,
            path: path,
            foldedName: ProjectPickerFolderSearchPath.fold(name),
            foldedSearchPath: ProjectPickerFolderSearchPath.foldedSearchPath(for: path, rootPath: rootURL.path)
        ))
    }

    private func shouldSkipDescendants(
        of url: URL,
        values: URLResourceValues,
        rootURL: URL
    ) -> Bool {
        if values.isHidden == true || values.isPackage == true || values.isSymbolicLink == true {
            return true
        }
        if Self.skippedNames.contains(url.lastPathComponent.lowercased()) {
            return true
        }
        let homeLibrary = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library").standardizedFileURL
        return rootURL.path == FileManager.default.homeDirectoryForCurrentUser.standardizedFileURL.path
            && url.path == homeLibrary.path
    }
}

private enum ProjectPickerFolderSearchFileSystemError: Error {
    case cannotEnumerate
}

actor ProjectPickerFolderSearchService {
    static let shared = ProjectPickerFolderSearchService()

    private static let defaultMaximumIndexedDirectoryCount = 50000
    private static let defaultMaximumVisitedEntryCount = 250_000
    private static let defaultMaximumScanDuration: Duration = .seconds(3)
    private static let defaultMaximumResultLimit = 100
    private static let defaultMaximumCachedRootCount = 2

    private let fileSystem: any ProjectPickerFolderSearchFileSystem
    private let watcherFactory: ProjectPickerFolderSearchWatcherFactory
    private let homeDirectory: String
    private let maximumIndexedDirectoryCount: Int
    private let maximumVisitedEntryCount: Int
    private let maximumScanDuration: Duration
    private let maximumResultLimit: Int
    private let maximumCachedRootCount: Int
    private var indexes: [String: ProjectPickerFolderSearchIndex] = [:]
    private var cacheOrder: [String] = []
    private var scans: [String: PendingScan] = [:]
    private var watchers: [String: any ProjectPickerFolderSearchWatching] = [:]

    init(
        fileSystem: any ProjectPickerFolderSearchFileSystem = FileManagerProjectPickerFolderSearchFileSystem(),
        watcherFactory: @escaping ProjectPickerFolderSearchWatcherFactory = { rootPath, handler in
            FileSystemWatcher(directoryPath: rootPath, handler: handler)
        },
        homeDirectory: String = NSHomeDirectory(),
        maximumIndexedDirectoryCount: Int = defaultMaximumIndexedDirectoryCount,
        maximumVisitedEntryCount: Int = defaultMaximumVisitedEntryCount,
        maximumScanDuration: Duration = defaultMaximumScanDuration,
        maximumResultLimit: Int = defaultMaximumResultLimit,
        maximumCachedRootCount: Int = defaultMaximumCachedRootCount
    ) {
        self.fileSystem = fileSystem
        self.watcherFactory = watcherFactory
        self.homeDirectory = ProjectPickerFolderSearchPath.standardizedAbsolutePath(homeDirectory) ?? homeDirectory
        self.maximumIndexedDirectoryCount = max(1, maximumIndexedDirectoryCount)
        self.maximumVisitedEntryCount = max(1, maximumVisitedEntryCount)
        self.maximumScanDuration = maximumScanDuration > .zero ? maximumScanDuration : .milliseconds(1)
        self.maximumResultLimit = max(1, maximumResultLimit)
        self.maximumCachedRootCount = max(1, maximumCachedRootCount)
    }

    func prepare(rootPath: String) async {
        guard let rootPath = ProjectPickerFolderSearchPath.standardizedAbsolutePath(rootPath, homeDirectory: homeDirectory) else {
            return
        }
        _ = await preparedIndex(rootPath: rootPath)
    }

    func search(
        query: String,
        rootPath: String,
        existingProjectPaths: [String],
        limit: Int
    ) async -> ProjectPickerFolderSearchSnapshot {
        guard let rootPath = ProjectPickerFolderSearchPath.standardizedAbsolutePath(rootPath, homeDirectory: homeDirectory) else {
            return failedSnapshot(.invalidRoot)
        }
        guard !Task.isCancelled else { return emptySnapshot() }

        switch await preparedIndex(rootPath: rootPath) {
        case let .ready(index):
            guard !Task.isCancelled else { return emptySnapshot(isTruncated: index.isTruncated) }
            return search(
                query: query,
                rootPath: rootPath,
                existingProjectPaths: existingProjectPaths,
                index: index,
                limit: limit
            )
        case let .failed(failure):
            return failedSnapshot(failure)
        case .cancelled:
            return emptySnapshot()
        }
    }

    func handleFileSystemChanges(_ paths: [String], rootPath: String) {
        guard let rootPath = ProjectPickerFolderSearchPath.standardizedAbsolutePath(rootPath, homeDirectory: homeDirectory)
        else { return }
        let shouldInvalidate = paths.contains { changedPath in
            guard let changedPath = ProjectPickerFolderSearchPath.standardizedAbsolutePath(
                changedPath,
                homeDirectory: homeDirectory
            ), ProjectPickerFolderSearchPath.isInside(changedPath, root: rootPath)
            else { return false }
            if changedPath == rootPath { return true }
            return fileSystem.itemState(atPath: changedPath) != .file
        }
        guard shouldInvalidate else { return }
        invalidate(rootPath: rootPath)
    }

    private func preparedIndex(rootPath: String) async -> PreparationOutcome {
        let rootState = await Task.detached(priority: .utility) { [fileSystem] in
            fileSystem.rootState(atPath: rootPath)
        }.value
        guard !Task.isCancelled else { return .cancelled }
        switch rootState {
        case .invalid:
            removeCachedIndex(for: rootPath)
            removeWatcher(for: rootPath)
            return .failed(.invalidRoot)
        case .unreadable:
            removeCachedIndex(for: rootPath)
            removeWatcher(for: rootPath)
            return .failed(.unreadableRoot)
        case .ready:
            break
        }

        ensureWatcher(for: rootPath)
        while !Task.isCancelled {
            if let index = indexes[rootPath] {
                touchCachedRoot(rootPath)
                return .ready(index)
            }

            let pendingScan = scans[rootPath] ?? startScan(rootPath: rootPath)
            let outcome = await pendingScan.task.value
            guard !Task.isCancelled else { return .cancelled }
            guard scans[rootPath]?.id == pendingScan.id else { continue }
            scans[rootPath] = nil
            switch outcome {
            case let .ready(index):
                store(index, for: rootPath)
                return .ready(index)
            case let .failed(failure):
                return .failed(failure)
            case .cancelled:
                return .cancelled
            }
        }
        return .cancelled
    }

    private func startScan(rootPath: String) -> PendingScan {
        let id = UUID()
        let fileSystem = fileSystem
        let maximumDirectoryCount = maximumIndexedDirectoryCount
        let maximumVisitedEntryCount = maximumVisitedEntryCount
        let maximumDuration = maximumScanDuration
        let task = Task.detached(priority: .utility) {
            do {
                let scannedIndex = try fileSystem.scanDirectories(
                    atPath: rootPath,
                    maximumDirectoryCount: maximumDirectoryCount,
                    maximumVisitedEntryCount: maximumVisitedEntryCount,
                    maximumDuration: maximumDuration
                )
                try Task.checkCancellation()
                let index = ProjectPickerFolderSearchPath.normalizedIndex(
                    scannedIndex,
                    rootPath: rootPath,
                    maximumCount: maximumDirectoryCount
                )
                return PreparationOutcome.ready(index)
            } catch is CancellationError {
                return PreparationOutcome.cancelled
            } catch {
                projectPickerFolderSearchLogger.error(
                    "Failed to index folders below \(rootPath, privacy: .private): \(error.localizedDescription, privacy: .public)"
                )
                return PreparationOutcome.failed(.scanFailed)
            }
        }
        let pendingScan = PendingScan(id: id, task: task)
        scans[rootPath] = pendingScan
        return pendingScan
    }

    private func search(
        query: String,
        rootPath: String,
        existingProjectPaths: [String],
        index: ProjectPickerFolderSearchIndex,
        limit: Int
    ) -> ProjectPickerFolderSearchSnapshot {
        guard let searchQuery = SearchQuery(rawValue: query) else {
            return emptySnapshot(isTruncated: index.isTruncated)
        }
        let effectiveLimit = min(max(0, limit), maximumResultLimit)
        guard effectiveLimit > 0 else { return emptySnapshot(isTruncated: index.isTruncated) }

        let existingPaths = normalizedExistingPaths(existingProjectPaths, rootPath: rootPath)
        var candidates: [String: SearchCandidate] = [:]
        candidates.reserveCapacity(min(index.entries.count, 4096))

        for path in existingPaths {
            guard candidates.count < maximumIndexedDirectoryCount else { break }
            let name = ProjectPickerFolderSearchPath.name(for: path)
            guard let match = SearchMatch(
                foldedName: ProjectPickerFolderSearchPath.fold(name),
                foldedSearchPath: ProjectPickerFolderSearchPath.foldedSearchPath(for: path, rootPath: rootPath),
                query: searchQuery
            )
            else {
                continue
            }
            candidates[path] = SearchCandidate(
                name: name,
                path: path,
                match: match,
                isExistingProject: true,
                depth: ProjectPickerFolderSearchPath.depth(of: path, below: rootPath)
            )
        }

        for entry in index.entries {
            guard !Task.isCancelled else { return emptySnapshot(isTruncated: index.isTruncated) }
            guard let match = SearchMatch(
                foldedName: entry.foldedName,
                foldedSearchPath: entry.foldedSearchPath,
                query: searchQuery
            )
            else { continue }
            if let existing = candidates[entry.path] {
                candidates[entry.path] = SearchCandidate(
                    name: entry.name,
                    path: entry.path,
                    match: min(existing.match, match),
                    isExistingProject: existing.isExistingProject,
                    depth: existing.depth
                )
                continue
            }
            guard candidates.count < maximumIndexedDirectoryCount else { break }
            candidates[entry.path] = SearchCandidate(
                name: entry.name,
                path: entry.path,
                match: match,
                isExistingProject: existingPaths.contains(entry.path),
                depth: ProjectPickerFolderSearchPath.depth(of: entry.path, below: rootPath)
            )
        }

        let ranked = candidates.values.sorted(by: SearchCandidate.precedes)
        let results = ranked.prefix(effectiveLimit).map {
            ProjectPickerFolderSearchResult(
                name: $0.name,
                path: $0.path,
                displayPath: ProjectPickerFolderSearchPath.displayPath($0.path, homeDirectory: homeDirectory)
            )
        }
        return ProjectPickerFolderSearchSnapshot(
            results: results,
            readFailed: false,
            isTruncated: index.isTruncated,
            hasMoreResults: ranked.count > effectiveLimit
        )
    }

    private func normalizedExistingPaths(_ paths: [String], rootPath: String) -> Set<String> {
        Set(paths.prefix(maximumIndexedDirectoryCount).compactMap {
            guard let path = ProjectPickerFolderSearchPath.standardizedAbsolutePath($0, homeDirectory: homeDirectory),
                  ProjectPickerFolderSearchPath.isInside(path, root: rootPath)
            else { return nil }
            return path
        })
    }

    private func store(_ index: ProjectPickerFolderSearchIndex, for rootPath: String) {
        indexes[rootPath] = index
        touchCachedRoot(rootPath)
        while cacheOrder.count > maximumCachedRootCount {
            let evictedRoot = cacheOrder[0]
            removeCachedIndex(for: evictedRoot)
            removeWatcher(for: evictedRoot)
        }
    }

    private func touchCachedRoot(_ rootPath: String) {
        cacheOrder.removeAll { $0 == rootPath }
        cacheOrder.append(rootPath)
    }

    private func removeCachedIndex(for rootPath: String) {
        indexes[rootPath] = nil
        cacheOrder.removeAll { $0 == rootPath }
    }

    private func ensureWatcher(for rootPath: String) {
        guard watchers[rootPath] == nil else { return }
        watchers[rootPath] = watcherFactory(rootPath) { [weak self] paths in
            Task { await self?.handleFileSystemChanges(paths, rootPath: rootPath) }
        }
    }

    private func invalidate(rootPath: String) {
        removeCachedIndex(for: rootPath)
        scans[rootPath]?.task.cancel()
        scans[rootPath] = nil
    }

    private func removeWatcher(for rootPath: String) {
        watchers[rootPath] = nil
        invalidate(rootPath: rootPath)
    }

    private func failedSnapshot(_ failure: ProjectPickerFolderSearchFailure) -> ProjectPickerFolderSearchSnapshot {
        ProjectPickerFolderSearchSnapshot(results: [], readFailed: true, failure: failure)
    }

    private func emptySnapshot(isTruncated: Bool = false) -> ProjectPickerFolderSearchSnapshot {
        ProjectPickerFolderSearchSnapshot(results: [], readFailed: false, isTruncated: isTruncated)
    }
}

private struct PendingScan {
    let id: UUID
    let task: Task<PreparationOutcome, Never>
}

private enum PreparationOutcome {
    case ready(ProjectPickerFolderSearchIndex)
    case failed(ProjectPickerFolderSearchFailure)
    case cancelled
}

private struct SearchQuery {
    let terms: [SearchTerm]

    init?(rawValue: String) {
        let separators = CharacterSet.whitespacesAndNewlines.union(CharacterSet(charactersIn: "/"))
        var seenTerms = Set<String>()
        terms = rawValue.components(separatedBy: separators).compactMap { value in
            let foldedValue = ProjectPickerFolderSearchPath.fold(value)
            guard !foldedValue.isEmpty, seenTerms.insert(foldedValue).inserted else { return nil }
            return SearchTerm(value: foldedValue)
        }
        guard !terms.isEmpty else { return nil }
    }
}

private struct SearchTerm {
    let value: String
    let exactComponent: String
    let componentPrefix: String

    init(value: String) {
        self.value = value
        exactComponent = "/\(value)/"
        componentPrefix = "/\(value)"
    }
}

private enum SearchMatchKind: Int, Comparable {
    case exact
    case prefix
    case substring
    case context

    init?(foldedName: String, term: String) {
        if foldedName == term {
            self = .exact
        } else if foldedName.hasPrefix(term) {
            self = .prefix
        } else if foldedName.contains(term) {
            self = .substring
        } else {
            return nil
        }
    }

    static func < (lhs: SearchMatchKind, rhs: SearchMatchKind) -> Bool {
        lhs.rawValue < rhs.rawValue
    }
}

private struct SearchMatch: Comparable {
    let kind: SearchMatchKind
    let pathScore: Int

    init?(foldedName: String, foldedSearchPath: String, query: SearchQuery) {
        if query.terms.count == 1 {
            guard let term = query.terms.first,
                  let kind = SearchMatchKind(foldedName: foldedName, term: term.value)
            else { return nil }
            self.kind = kind
            pathScore = kind.rawValue
            return
        }

        var pathScore = 0
        for term in query.terms {
            if foldedSearchPath.contains(term.exactComponent) {
                continue
            }
            if foldedSearchPath.contains(term.componentPrefix) {
                pathScore += SearchMatchKind.prefix.rawValue
                continue
            }
            guard foldedSearchPath.contains(term.value) else { return nil }
            pathScore += SearchMatchKind.substring.rawValue
        }

        kind = query.terms.compactMap {
            SearchMatchKind(foldedName: foldedName, term: $0.value)
        }.min() ?? .context
        self.pathScore = pathScore
    }

    static func < (lhs: SearchMatch, rhs: SearchMatch) -> Bool {
        if lhs.kind != rhs.kind { return lhs.kind < rhs.kind }
        return lhs.pathScore < rhs.pathScore
    }
}

private struct SearchCandidate {
    let name: String
    let path: String
    let match: SearchMatch
    let isExistingProject: Bool
    let depth: Int

    static func precedes(_ lhs: SearchCandidate, _ rhs: SearchCandidate) -> Bool {
        if lhs.match != rhs.match { return lhs.match < rhs.match }
        if lhs.isExistingProject != rhs.isExistingProject { return lhs.isExistingProject }
        if lhs.depth != rhs.depth { return lhs.depth < rhs.depth }
        let nameOrder = lhs.name.localizedStandardCompare(rhs.name)
        if nameOrder != .orderedSame { return nameOrder == .orderedAscending }
        return lhs.path.localizedStandardCompare(rhs.path) == .orderedAscending
    }
}

private enum ProjectPickerFolderSearchPath {
    static func standardizedAbsolutePath(_ path: String, homeDirectory: String = NSHomeDirectory()) -> String? {
        let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedPath.isEmpty else { return nil }
        let expandedPath: String = if trimmedPath == "~" {
            homeDirectory
        } else if trimmedPath.hasPrefix("~/") {
            homeDirectory + trimmedPath.dropFirst()
        } else {
            trimmedPath
        }
        guard expandedPath.hasPrefix("/") else { return nil }
        return URL(fileURLWithPath: expandedPath, isDirectory: true).standardizedFileURL.path
    }

    static func fold(_ value: String) -> String {
        value.folding(
            options: [.caseInsensitive, .diacriticInsensitive, .widthInsensitive],
            locale: .current
        ).lowercased()
    }

    static func name(for path: String) -> String {
        guard path != "/" else { return "/" }
        return URL(fileURLWithPath: path, isDirectory: true).lastPathComponent
    }

    static func isInside(_ path: String, root: String) -> Bool {
        if root == "/" { return path.hasPrefix("/") }
        return path == root || path.hasPrefix(root + "/")
    }

    static func depth(of path: String, below root: String) -> Int {
        max(0, URL(fileURLWithPath: path).pathComponents.count - URL(fileURLWithPath: root).pathComponents.count)
    }

    static func foldedSearchPath(for path: String, rootPath: String) -> String {
        let relativePath: Substring = if path == rootPath {
            Substring(name(for: path))
        } else if rootPath == "/" {
            path.dropFirst()
        } else {
            path.dropFirst(rootPath.count + 1)
        }
        return "/\(fold(relativePath.description))/"
    }

    static func displayPath(_ path: String, homeDirectory: String) -> String {
        let displayPath: String = if path == homeDirectory {
            "~"
        } else if path.hasPrefix(homeDirectory + "/") {
            "~" + path.dropFirst(homeDirectory.count)
        } else {
            path
        }
        if displayPath == "/" || displayPath.hasSuffix("/") { return displayPath }
        return displayPath + "/"
    }

    static func normalizedIndex(
        _ index: ProjectPickerFolderSearchIndex,
        rootPath: String,
        maximumCount: Int
    ) -> ProjectPickerFolderSearchIndex {
        var entriesByPath: [String: ProjectPickerFolderSearchIndexEntry] = [:]
        entriesByPath.reserveCapacity(min(index.entries.count, maximumCount))
        var isTruncated = index.isTruncated
        for entry in index.entries {
            guard let path = standardizedAbsolutePath(entry.path), isInside(path, root: rootPath) else { continue }
            guard entriesByPath[path] == nil else { continue }
            guard entriesByPath.count < maximumCount else {
                isTruncated = true
                break
            }
            let name = name(for: path)
            entriesByPath[path] = ProjectPickerFolderSearchIndexEntry(
                name: name,
                path: path,
                foldedName: fold(name),
                foldedSearchPath: foldedSearchPath(for: path, rootPath: rootPath)
            )
        }
        return ProjectPickerFolderSearchIndex(
            entries: entriesByPath.values.sorted { $0.path < $1.path },
            isTruncated: isTruncated
        )
    }
}
