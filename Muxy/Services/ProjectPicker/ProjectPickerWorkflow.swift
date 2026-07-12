import Foundation

typealias ProjectPickerDirectoryLoader = @Sendable (ProjectPickerPathState) async -> ProjectPickerDirectorySnapshot
typealias ProjectPickerDirectoryItemsLoader = @Sendable (String) async -> [ProjectPickerDirectoryItem]?
typealias ProjectPickerFolderSearchPreparer = @Sendable (String) async -> Void
typealias ProjectPickerFolderSearchLoader = @Sendable (
    _ query: String,
    _ rootPath: String,
    _ existingProjectPaths: [String],
    _ limit: Int
) async -> ProjectPickerFolderSearchSnapshot

@MainActor
@Observable
final class ProjectPickerWorkflow {
    private(set) var session: ProjectPickerSession

    @ObservationIgnored private var directoryLoadID = UUID()
    @ObservationIgnored private var reloadTask: Task<Void, Never>?
    @ObservationIgnored private var folderSearchPreparationTask: Task<Void, Never>?
    @ObservationIgnored private var loadingMessageTask: Task<Void, Never>?
    @ObservationIgnored private let directoryLoader: ProjectPickerDirectoryLoader?
    @ObservationIgnored private let itemsLoader: ProjectPickerDirectoryItemsLoader
    @ObservationIgnored private let folderSearchPreparer: ProjectPickerFolderSearchPreparer?
    @ObservationIgnored private let folderSearchLoader: ProjectPickerFolderSearchLoader?
    @ObservationIgnored private let reloadDelay: Duration
    @ObservationIgnored private let loadingMessageDelay: Duration
    @ObservationIgnored private var didAppear = false
    @ObservationIgnored private var directoryCache: [String: [ProjectPickerDirectoryItem]] = [:]
    @ObservationIgnored private var directoryCacheOrder: [String] = []
    private static let directoryCacheLimit = 64
    private static let folderSearchResultLimit = 50

    init(
        defaultDisplayPath: String = ProjectPickerDefaultLocation.state.displayPath,
        homeDirectory: String = NSHomeDirectory(),
        projectPaths: [String],
        directoryLoader: ProjectPickerDirectoryLoader? = nil,
        folderSearchPreparer: ProjectPickerFolderSearchPreparer? = nil,
        folderSearchLoader: ProjectPickerFolderSearchLoader? = nil,
        reloadDelay: Duration = .milliseconds(100),
        loadingMessageDelay: Duration = .milliseconds(500)
    ) {
        let session = ProjectPickerSession(
            defaultDisplayPath: defaultDisplayPath,
            homeDirectory: homeDirectory,
            projectPaths: projectPaths
        )
        self.session = session
        self.directoryLoader = directoryLoader
        itemsLoader = Self.itemsLoader(for: session.pathService)
        self.folderSearchPreparer = folderSearchPreparer ?? Self.liveFolderSearchPreparer
        self.folderSearchLoader = folderSearchLoader ?? Self.liveFolderSearchLoader
        self.reloadDelay = reloadDelay
        self.loadingMessageDelay = loadingMessageDelay
    }

    init(
        projectPaths: [String],
        context: WorkspaceContext,
        reloadDelay: Duration = .milliseconds(100),
        loadingMessageDelay: Duration = .milliseconds(500)
    ) {
        let session = ProjectPickerSession(projectPaths: projectPaths, context: context)
        self.session = session
        directoryLoader = nil
        itemsLoader = Self.itemsLoader(for: session.pathService)
        folderSearchPreparer = context.isRemote ? nil : Self.liveFolderSearchPreparer
        folderSearchLoader = context.isRemote ? nil : Self.liveFolderSearchLoader
        self.reloadDelay = reloadDelay
        self.loadingMessageDelay = loadingMessageDelay
    }

    func appear() {
        guard !didAppear else { return }
        didAppear = true
        guard session.allowsFolderSearch else {
            scheduleReload()
            return
        }
        session.applyFolderSearchSnapshot(ProjectPickerFolderSearchSnapshot(results: [], readFailed: false))
        guard let folderSearchPreparer else { return }
        let rootPath = session.searchRootPath
        folderSearchPreparationTask = Task(priority: .utility) {
            await folderSearchPreparer(rootPath)
        }
    }

    func cancel() {
        cancelReload()
        folderSearchPreparationTask?.cancel()
        folderSearchPreparationTask = nil
    }

    func setProjectPaths(_ projectPaths: [String]) {
        session.setProjectPaths(projectPaths)
    }

    func setInput(_ input: String) -> [ProjectPickerWorkflowRequest] {
        session.setInput(input)
        scheduleReload()
        return []
    }

    func selectRow(at index: Int) {
        session.selectRow(at: index)
    }

    func activate(row: ProjectPickerDirectoryItem) -> [ProjectPickerWorkflowRequest] {
        reloadAfterInputChange {
            session.activate(row: row)
        }
    }

    func activate(searchResult: ProjectPickerFolderSearchResult) -> [ProjectPickerWorkflowRequest] {
        [.confirmProjectPath(path: searchResult.path, createIfMissing: false)]
    }

    func handle(_ command: ProjectPickerCommand) -> [ProjectPickerWorkflowRequest] {
        switch command {
        case .moveHighlightUp,
             .moveHighlightDown:
            session.handle(command)
            return []
        case .openHighlighted:
            guard session.inputMode == .path else { return confirmHighlightedSearchResult() }
            return reloadAfterInputChange {
                session.handle(command)
            }
        case .goBack,
             .completeHighlighted:
            return reloadAfterInputChange {
                session.handle(command)
            }
        case .confirmTypedPath:
            return confirmSelection()
        case .dismiss:
            return [.dismiss]
        }
    }

    func chooseWithFinder() -> [ProjectPickerWorkflowRequest] {
        [.dismiss, .chooseFinder]
    }

    func editDefaultLocation() -> [ProjectPickerWorkflowRequest] {
        [.dismiss, .openSettingsFocusedOnDefaultLocation]
    }

    func handleCreateDirectoryDecision(path: String, accepted: Bool) -> [ProjectPickerWorkflowRequest] {
        guard accepted else { return [] }
        return [.confirmProjectPath(path: path, createIfMissing: true)]
    }

    func handleProjectPathConfirmationResult(
        _ result: ProjectOpenConfirmationResult,
        path: String
    ) -> [ProjectPickerWorkflowRequest] {
        guard !result.didConfirm else { return [.dismiss] }
        return [.showFailure(ProjectPickerConfirmationFailurePresentation(result: result, path: path))]
    }

    private func confirmSelection() -> [ProjectPickerWorkflowRequest] {
        guard session.inputMode == .path else { return confirmHighlightedSearchResult() }
        let path = session.standardizedTypedPath
        guard session.typedPathState != .missing else {
            return [.askCreateDirectory(path: path)]
        }
        return [.confirmProjectPath(path: path, createIfMissing: false)]
    }

    private func confirmHighlightedSearchResult() -> [ProjectPickerWorkflowRequest] {
        guard let path = session.highlightedSearchResult?.path else { return [] }
        return [.confirmProjectPath(path: path, createIfMissing: false)]
    }

    private func reloadAfterInputChange(_ update: () -> Void) -> [ProjectPickerWorkflowRequest] {
        let previousInput = session.input
        update()
        guard session.input != previousInput else { return [] }
        scheduleReload()
        return []
    }

    private func scheduleReload() {
        cancelReload()
        let loadID = UUID()
        directoryLoadID = loadID

        guard session.inputMode == .path else {
            scheduleFolderSearch(loadID: loadID)
            return
        }
        scheduleDirectoryReload(pathState: session.pathState, loadID: loadID)
    }

    private func scheduleFolderSearch(loadID: UUID) {
        let query = session.searchQuery
        guard !query.isEmpty else {
            applyFolderSearchSnapshot(
                ProjectPickerFolderSearchSnapshot(results: [], readFailed: false),
                loadID: loadID
            )
            return
        }

        scheduleLoadingMessage(loadID: loadID)
        guard let folderSearchLoader else {
            applyFolderSearchSnapshot(
                ProjectPickerFolderSearchSnapshot(results: [], readFailed: true),
                loadID: loadID
            )
            return
        }
        let rootPath = session.searchRootPath
        let projectPaths = session.projectPaths
        reloadTask = Task { [weak self, reloadDelay] in
            try? await Task.sleep(for: reloadDelay)
            guard !Task.isCancelled else { return }
            let snapshot = await folderSearchLoader(query, rootPath, projectPaths, Self.folderSearchResultLimit)
            guard !Task.isCancelled else { return }
            self?.applyFolderSearchSnapshot(snapshot, loadID: loadID)
        }
    }

    private func scheduleDirectoryReload(pathState: ProjectPickerPathState, loadID: UUID) {
        if let cached = directoryCache[pathState.directoryPath] {
            let snapshot = session.pathService.snapshot(for: pathState, items: cached)
            applyDirectorySnapshot(snapshot, loadID: loadID)
            return
        }

        scheduleLoadingMessage(loadID: loadID)

        if let directoryLoader {
            reloadTask = Task { [weak self, reloadDelay] in
                try? await Task.sleep(for: reloadDelay)
                guard !Task.isCancelled else { return }
                let snapshot = await directoryLoader(pathState)
                guard !Task.isCancelled else { return }
                self?.applyDirectorySnapshot(snapshot, loadID: loadID)
            }
            return
        }

        reloadTask = Task { [weak self, reloadDelay, itemsLoader] in
            try? await Task.sleep(for: reloadDelay)
            guard !Task.isCancelled else { return }
            let items = await itemsLoader(pathState.directoryPath)
            guard !Task.isCancelled else { return }
            self?.applyItems(items, pathState: pathState, loadID: loadID)
        }
    }

    private func scheduleLoadingMessage(loadID: UUID) {
        loadingMessageTask = Task { [weak self, loadingMessageDelay] in
            try? await Task.sleep(for: loadingMessageDelay)
            guard !Task.isCancelled else { return }
            self?.showLoadingMessage(loadID: loadID)
        }
    }

    private func applyItems(
        _ items: [ProjectPickerDirectoryItem]?,
        pathState: ProjectPickerPathState,
        loadID: UUID
    ) {
        guard directoryLoadID == loadID else { return }
        guard let items else {
            let snapshot = ProjectPickerDirectorySnapshot(rows: pathState.directoryReadFailureItems, readFailed: true)
            applyDirectorySnapshot(snapshot, loadID: loadID)
            return
        }
        cacheItems(items, for: pathState.directoryPath)
        applyDirectorySnapshot(session.pathService.snapshot(for: pathState, items: items), loadID: loadID)
    }

    private func cacheItems(_ items: [ProjectPickerDirectoryItem], for directoryPath: String) {
        if directoryCache[directoryPath] == nil {
            directoryCacheOrder.append(directoryPath)
        }
        directoryCache[directoryPath] = items
        while directoryCacheOrder.count > Self.directoryCacheLimit {
            let evicted = directoryCacheOrder.removeFirst()
            directoryCache[evicted] = nil
        }
    }

    private func cancelReload() {
        reloadTask?.cancel()
        loadingMessageTask?.cancel()
        reloadTask = nil
        loadingMessageTask = nil
    }

    private func showLoadingMessage(loadID: UUID) {
        guard directoryLoadID == loadID else { return }
        session.showLoadingMessage()
    }

    private func applyDirectorySnapshot(_ snapshot: ProjectPickerDirectorySnapshot, loadID: UUID) {
        guard directoryLoadID == loadID else { return }
        loadingMessageTask?.cancel()
        loadingMessageTask = nil
        session.applyDirectorySnapshot(snapshot)
    }

    private func applyFolderSearchSnapshot(_ snapshot: ProjectPickerFolderSearchSnapshot, loadID: UUID) {
        guard directoryLoadID == loadID else { return }
        loadingMessageTask?.cancel()
        loadingMessageTask = nil
        session.applyFolderSearchSnapshot(snapshot)
    }

    private static func itemsLoader(for pathService: ProjectPickerPathService) -> ProjectPickerDirectoryItemsLoader {
        { directoryPath in
            switch await pathService.directoryContents(atPath: directoryPath) {
            case let .success(items): items
            case .failure: nil
            }
        }
    }

    private static let liveFolderSearchPreparer: ProjectPickerFolderSearchPreparer = { rootPath in
        await ProjectPickerFolderSearchService.shared.prepare(rootPath: rootPath)
    }

    private static let liveFolderSearchLoader: ProjectPickerFolderSearchLoader = { query, rootPath, projectPaths, limit in
        await ProjectPickerFolderSearchService.shared.search(
            query: query,
            rootPath: rootPath,
            existingProjectPaths: projectPaths,
            limit: limit
        )
    }
}

enum ProjectPickerWorkflowRequest: Equatable {
    case askCreateDirectory(path: String)
    case confirmProjectPath(path: String, createIfMissing: Bool)
    case chooseFinder
    case openSettingsFocusedOnDefaultLocation
    case dismiss
    case showFailure(ProjectPickerConfirmationFailurePresentation)
}
