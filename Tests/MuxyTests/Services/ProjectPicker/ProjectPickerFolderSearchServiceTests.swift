import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerFolderSearchService")
struct ProjectPickerFolderSearchServiceTests {
    @Test("indexes skipped folders without descending into hidden generated or package contents")
    func indexesSkippedFoldersWithoutDescendants() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-folder-search-\(UUID().uuidString)", isDirectory: true)
        let visibleMatch = root.appendingPathComponent("Sources/needle-visible", isDirectory: true)
        let hidden = root.appendingPathComponent(".needle-hidden", isDirectory: true)
        let hiddenMatch = hidden.appendingPathComponent("needle-hidden-child", isDirectory: true)
        let generated = root.appendingPathComponent("node_modules", isDirectory: true)
        let generatedMatch = generated.appendingPathComponent("needle-generated-child", isDirectory: true)
        let package = root.appendingPathComponent("Needle.app", isDirectory: true)
        let packageMatch = package.appendingPathComponent("Contents/needle-package-child", isDirectory: true)
        for directory in [visibleMatch, hiddenMatch, generatedMatch, packageMatch] {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        defer { try? FileManager.default.removeItem(at: root) }

        let packageValues = try package.resourceValues(forKeys: [.isPackageKey])
        try #require(packageValues.isPackage == true)
        let service = ProjectPickerFolderSearchService(watcherFactory: { _, _ in nil })
        let snapshot = await service.search(
            query: "needle",
            rootPath: root.path,
            existingProjectPaths: [],
            limit: 100
        )
        let names = Set(snapshot.results.map(\.name))

        #expect(!snapshot.readFailed)
        #expect(names.contains("needle-visible"))
        #expect(names.contains(".needle-hidden"))
        #expect(names.contains("Needle.app"))
        #expect(!names.contains("needle-hidden-child"))
        #expect(!names.contains("needle-generated-child"))
        #expect(!names.contains("needle-package-child"))

        let generatedSnapshot = await service.search(
            query: "node_modules",
            rootPath: root.path,
            existingProjectPaths: [],
            limit: 100
        )
        #expect(generatedSnapshot.results.map(\.name) == ["node_modules"])
    }

    @Test("ranks exact existing prefix and substring matches with a hard result cap")
    func ranksAndCapsResults() async {
        let root = "/workspace"
        let fileSystem = FolderSearchFileSystemStub(
            rootStates: [root: .ready],
            indexes: [root: ProjectPickerFolderSearchIndex(entries: [
                entry("/workspace/my-muxy-copy"),
                entry("/workspace/muxy-tools"),
                entry("/workspace/muxy"),
                entry("/workspace/group/../muxy"),
                entry("/workspace/clients/muxy"),
            ], isTruncated: false)]
        )
        let service = ProjectPickerFolderSearchService(
            fileSystem: fileSystem,
            watcherFactory: { _, _ in nil },
            homeDirectory: "/Users/alice",
            maximumResultLimit: 3
        )

        let snapshot = await service.search(
            query: "muxy",
            rootPath: root,
            existingProjectPaths: ["/workspace/clients/muxy"],
            limit: 20
        )

        #expect(snapshot.results.map(\.path) == [
            "/workspace/clients/muxy",
            "/workspace/muxy",
            "/workspace/muxy-tools",
        ])
        #expect(snapshot.results[0].name == "muxy")
        #expect(snapshot.results[0].displayPath == "/workspace/clients/muxy/")
        #expect(snapshot.hasMoreResults)
    }

    @Test("matches unordered parent and folder terms separated by slashes or spaces")
    func matchesPathTerms() async {
        let root = "/Users/alice"
        let target = "/Users/alice/Projects/capty/app"
        let capty = "/Users/alice/Projects/capty"
        let fileSystem = FolderSearchFileSystemStub(
            rootStates: [root: .ready],
            indexes: [root: ProjectPickerFolderSearchIndex(entries: [
                entry(capty),
                entry(target),
                entry("/Users/alice/Projects/capty/apple"),
                entry("/Users/alice/Projects/my-capty/application"),
                entry("/Users/alice/Projects/other/app"),
            ], isTruncated: false)]
        )
        let service = ProjectPickerFolderSearchService(
            fileSystem: fileSystem,
            watcherFactory: { _, _ in nil },
            homeDirectory: root
        )

        for query in ["capty/app", "capty app", "app capty"] {
            let snapshot = await service.search(
                query: query,
                rootPath: root,
                existingProjectPaths: [],
                limit: 100
            )

            #expect(snapshot.results.first?.path == target)
            #expect(!snapshot.results.map(\.path).contains("/Users/alice/Projects/other/app"))
        }

        let singleTermSnapshot = await service.search(
            query: "capty",
            rootPath: root,
            existingProjectPaths: [],
            limit: 100
        )

        #expect(singleTermSnapshot.results.first?.path == capty)
        #expect(!singleTermSnapshot.results.map(\.path).contains(target))
    }

    @Test("reports invalid and unreadable roots without scanning")
    func reportsRootFailures() async {
        let fileSystem = FolderSearchFileSystemStub(rootStates: [
            "/missing": .invalid,
            "/private": .unreadable,
        ])
        let service = ProjectPickerFolderSearchService(fileSystem: fileSystem, watcherFactory: { _, _ in nil })

        let malformed = await service.search(query: "muxy", rootPath: "", existingProjectPaths: [], limit: 10)
        let missing = await service.search(query: "muxy", rootPath: "/missing", existingProjectPaths: [], limit: 10)
        let unreadable = await service.search(query: "muxy", rootPath: "/private", existingProjectPaths: [], limit: 10)

        #expect(malformed.failure == .invalidRoot)
        #expect(missing.failure == .invalidRoot)
        #expect(unreadable.failure == .unreadableRoot)
        #expect(malformed.readFailed && missing.readFailed && unreadable.readFailed)
        #expect(fileSystem.scanCount == 0)
    }

    @Test("ignores file writes and refreshes after a directory change")
    func reusesAndInvalidatesRootIndex() async throws {
        let root = "/workspace"
        let fileSystem = FolderSearchFileSystemStub(
            rootStates: [root: .ready],
            itemStates: [
                "/workspace/file.swift": .file,
                "/workspace/new-folder": .directory,
            ],
            indexes: [root: ProjectPickerFolderSearchIndex(entries: [entry("/workspace/first")], isTruncated: false)]
        )
        let service = ProjectPickerFolderSearchService(
            fileSystem: fileSystem,
            watcherFactory: { _, _ in nil }
        )

        let first = await service.search(query: "first", rootPath: root, existingProjectPaths: [], limit: 10)
        _ = await service.search(query: "first", rootPath: root, existingProjectPaths: [], limit: 10)
        #expect(first.results.map(\.name) == ["first"])
        #expect(fileSystem.scanCount == 1)

        fileSystem.setIndex(
            ProjectPickerFolderSearchIndex(entries: [entry("/workspace/second")], isTruncated: false),
            for: root
        )
        await service.handleFileSystemChanges(["/workspace/file.swift"], rootPath: root)
        let unchanged = await service.search(query: "first", rootPath: root, existingProjectPaths: [], limit: 10)
        #expect(unchanged.results.map(\.name) == ["first"])
        #expect(fileSystem.scanCount == 1)

        await service.handleFileSystemChanges(["/workspace/new-folder"], rootPath: root)

        let second = await service.search(query: "second", rootPath: root, existingProjectPaths: [], limit: 10)
        #expect(second.results.map(\.name) == ["second"])
        #expect(fileSystem.scanCount == 2)
    }

    @Test("canceling one waiter does not cancel a shared root scan")
    func cancellationDoesNotCancelSharedScan() async {
        let root = "/workspace"
        let gate = FolderSearchScanGate(index: ProjectPickerFolderSearchIndex(
            entries: [entry("/workspace/muxy")],
            isTruncated: false
        ))
        let fileSystem = FolderSearchFileSystemStub(
            rootStates: [root: .ready],
            scanHandler: { _, _, _, _ in try gate.scan() }
        )
        let service = ProjectPickerFolderSearchService(fileSystem: fileSystem, watcherFactory: { _, _ in nil })
        let firstTask = Task {
            await service.search(query: "muxy", rootPath: root, existingProjectPaths: [], limit: 10)
        }

        await gate.waitUntilStarted()
        let secondTask = Task {
            await service.search(query: "muxy", rootPath: root, existingProjectPaths: [], limit: 10)
        }
        firstTask.cancel()
        gate.release()

        let first = await firstTask.value
        let second = await secondTask.value
        #expect(first.results.isEmpty)
        #expect(!first.readFailed)
        #expect(second.results.map(\.name) == ["muxy"])
        #expect(fileSystem.scanCount == 1)
        #expect(!gate.wasCancelled)
    }

    @Test("retries when a directory change invalidates an in-flight scan")
    func invalidationRetriesInFlightScan() async {
        let root = "/workspace"
        let controller = FolderSearchInvalidationController(freshIndex: ProjectPickerFolderSearchIndex(
            entries: [entry("/workspace/fresh")],
            isTruncated: false
        ))
        let fileSystem = FolderSearchFileSystemStub(
            rootStates: [root: .ready],
            itemStates: ["/workspace/new-folder": .directory],
            scanHandler: controller.scan
        )
        let service = ProjectPickerFolderSearchService(fileSystem: fileSystem, watcherFactory: { _, _ in nil })
        let searchTask = Task {
            await service.search(query: "fresh", rootPath: root, existingProjectPaths: [], limit: 10)
        }

        await controller.waitUntilFirstScanStarts()
        await service.handleFileSystemChanges(["/workspace/new-folder"], rootPath: root)
        let snapshot = await searchTask.value

        #expect(snapshot.results.map(\.name) == ["fresh"])
        #expect(fileSystem.scanCount == 2)
        #expect(controller.firstScanWasCancelled)
    }

    @Test("caps indexed folders and exposes truncation")
    func capsIndexedFolders() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-folder-search-cap-\(UUID().uuidString)", isDirectory: true)
        for name in ["folder-a", "folder-b", "folder-c", "folder-d"] {
            try FileManager.default.createDirectory(
                at: root.appendingPathComponent(name, isDirectory: true),
                withIntermediateDirectories: true
            )
        }
        defer { try? FileManager.default.removeItem(at: root) }
        let service = ProjectPickerFolderSearchService(
            watcherFactory: { _, _ in nil },
            maximumIndexedDirectoryCount: 3
        )

        let snapshot = await service.search(
            query: "folder",
            rootPath: root.path,
            existingProjectPaths: [],
            limit: 100
        )

        #expect(snapshot.isTruncated)
        #expect(snapshot.results.count <= 3)
    }

    @Test("stops scanning when the visited entry or elapsed time budget is exhausted")
    func enforcesTraversalBudgets() async throws {
        let root = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-folder-search-budget-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        for index in 0 ..< 10 {
            try Data().write(to: root.appendingPathComponent("file-\(index)"))
        }
        defer { try? FileManager.default.removeItem(at: root) }

        let visitedEntryService = ProjectPickerFolderSearchService(
            watcherFactory: { _, _ in nil },
            maximumVisitedEntryCount: 2
        )
        let visitedEntrySnapshot = await visitedEntryService.search(
            query: "folder",
            rootPath: root.path,
            existingProjectPaths: [],
            limit: 100
        )
        let elapsedTimeService = ProjectPickerFolderSearchService(
            watcherFactory: { _, _ in nil },
            maximumScanDuration: .nanoseconds(1)
        )
        let elapsedTimeSnapshot = await elapsedTimeService.search(
            query: "folder",
            rootPath: root.path,
            existingProjectPaths: [],
            limit: 100
        )

        #expect(visitedEntrySnapshot.isTruncated)
        #expect(elapsedTimeSnapshot.isTruncated)
    }

    private func entry(_ path: String) -> ProjectPickerFolderSearchIndexEntry {
        let name = URL(fileURLWithPath: path).lastPathComponent
        return ProjectPickerFolderSearchIndexEntry(
            name: name,
            path: path,
            foldedName: name.lowercased(),
            foldedSearchPath: path.lowercased()
        )
    }
}

private final class FolderSearchFileSystemStub: ProjectPickerFolderSearchFileSystem, @unchecked Sendable {
    typealias ScanHandler = @Sendable (String, Int, Int, Duration) throws -> ProjectPickerFolderSearchIndex

    private let lock = NSLock()
    private let rootStates: [String: ProjectPickerFolderSearchRootState]
    private let itemStates: [String: ProjectPickerFolderSearchItemState]
    private var indexes: [String: ProjectPickerFolderSearchIndex]
    private let scanHandler: ScanHandler?
    private var storedScanCount = 0

    init(
        rootStates: [String: ProjectPickerFolderSearchRootState],
        itemStates: [String: ProjectPickerFolderSearchItemState] = [:],
        indexes: [String: ProjectPickerFolderSearchIndex] = [:],
        scanHandler: ScanHandler? = nil
    ) {
        self.rootStates = rootStates
        self.itemStates = itemStates
        self.indexes = indexes
        self.scanHandler = scanHandler
    }

    var scanCount: Int {
        lock.withLock { storedScanCount }
    }

    func rootState(atPath path: String) -> ProjectPickerFolderSearchRootState {
        rootStates[path] ?? .invalid
    }

    func itemState(atPath path: String) -> ProjectPickerFolderSearchItemState {
        itemStates[path] ?? .missing
    }

    func scanDirectories(
        atPath path: String,
        maximumDirectoryCount: Int,
        maximumVisitedEntryCount: Int,
        maximumDuration: Duration
    ) throws -> ProjectPickerFolderSearchIndex {
        let index = lock.withLock { () -> ProjectPickerFolderSearchIndex? in
            storedScanCount += 1
            return indexes[path]
        }
        if let scanHandler {
            return try scanHandler(path, maximumDirectoryCount, maximumVisitedEntryCount, maximumDuration)
        }
        guard let index else { throw FolderSearchFileSystemStubError.missingIndex }
        return index
    }

    func setIndex(_ index: ProjectPickerFolderSearchIndex, for rootPath: String) {
        lock.withLock {
            indexes[rootPath] = index
        }
    }
}

private enum FolderSearchFileSystemStubError: Error {
    case missingIndex
}

private final class FolderSearchScanGate: @unchecked Sendable {
    private let condition = NSCondition()
    private let index: ProjectPickerFolderSearchIndex
    private var started = false
    private var released = false
    private var cancelled = false

    init(index: ProjectPickerFolderSearchIndex) {
        self.index = index
    }

    var wasCancelled: Bool {
        condition.withLock { cancelled }
    }

    func scan() throws -> ProjectPickerFolderSearchIndex {
        condition.lock()
        started = true
        condition.broadcast()
        while !released {
            if Task.isCancelled {
                cancelled = true
                condition.unlock()
                throw CancellationError()
            }
            _ = condition.wait(until: Date().addingTimeInterval(0.01))
        }
        condition.unlock()
        return index
    }

    func release() {
        condition.withLock {
            released = true
            condition.broadcast()
        }
    }

    func waitUntilStarted() async {
        while !condition.withLock({ started }) {
            await Task.yield()
        }
    }
}

private final class FolderSearchInvalidationController: @unchecked Sendable {
    private let condition = NSCondition()
    private let freshIndex: ProjectPickerFolderSearchIndex
    private var scanCount = 0
    private var firstScanStarted = false
    private var firstScanCancelled = false

    init(freshIndex: ProjectPickerFolderSearchIndex) {
        self.freshIndex = freshIndex
    }

    var firstScanWasCancelled: Bool {
        condition.withLock { firstScanCancelled }
    }

    func scan(
        path _: String,
        maximumDirectoryCount _: Int,
        maximumVisitedEntryCount _: Int,
        maximumDuration _: Duration
    ) throws -> ProjectPickerFolderSearchIndex {
        condition.lock()
        scanCount += 1
        guard scanCount == 1 else {
            condition.unlock()
            return freshIndex
        }
        firstScanStarted = true
        condition.broadcast()
        while !Task.isCancelled {
            _ = condition.wait(until: Date().addingTimeInterval(0.01))
        }
        firstScanCancelled = true
        condition.unlock()
        throw CancellationError()
    }

    func waitUntilFirstScanStarts() async {
        while !condition.withLock({ firstScanStarted }) {
            await Task.yield()
        }
    }
}
