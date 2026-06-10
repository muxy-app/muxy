import Foundation

typealias ProjectPickerDirectoryLoader = @Sendable (ProjectPickerPathState) async -> ProjectPickerDirectorySnapshot
typealias ProjectPickerDirectoryItemsLoader = @Sendable (String) async -> [ProjectPickerDirectoryItem]?

@MainActor
@Observable
final class ProjectPickerWorkflow {
    private(set) var session: ProjectPickerSession

    @ObservationIgnored private var directoryLoadID = UUID()
    @ObservationIgnored private var reloadTask: Task<Void, Never>?
    @ObservationIgnored private var loadingMessageTask: Task<Void, Never>?
    @ObservationIgnored private let directoryLoader: ProjectPickerDirectoryLoader?
    @ObservationIgnored private let itemsLoader: ProjectPickerDirectoryItemsLoader
    @ObservationIgnored private let reloadDelay: Duration
    @ObservationIgnored private let loadingMessageDelay: Duration
    @ObservationIgnored private var didAppear = false
    @ObservationIgnored private var directoryCache: [String: [ProjectPickerDirectoryItem]] = [:]
    @ObservationIgnored private var directoryCacheOrder: [String] = []
    private static let directoryCacheLimit = 64

    init(
        defaultDisplayPath: String = ProjectPickerDefaultLocation.state.displayPath,
        homeDirectory: String = NSHomeDirectory(),
        projectPaths: [String],
        directoryLoader: ProjectPickerDirectoryLoader? = nil,
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
        self.reloadDelay = reloadDelay
        self.loadingMessageDelay = loadingMessageDelay
    }

    func appear() {
        guard !didAppear else { return }
        didAppear = true
        scheduleDirectoryReload(pathState: session.pathState)
    }

    func cancel() {
        cancelDirectoryReload()
    }

    func setProjectPaths(_ projectPaths: [String]) {
        session.setProjectPaths(projectPaths)
    }

    func setInput(_ input: String) -> [ProjectPickerWorkflowRequest] {
        session.setInput(input)
        scheduleDirectoryReload(pathState: session.pathState)
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

    func handle(_ command: ProjectPickerCommand) -> [ProjectPickerWorkflowRequest] {
        switch command {
        case .moveHighlightUp,
             .moveHighlightDown:
            session.handle(command)
            return []
        case .openHighlighted,
             .goBack,
             .completeHighlighted:
            return reloadAfterInputChange {
                session.handle(command)
            }
        case .confirmTypedPath:
            return confirmTypedPath()
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

    private func confirmTypedPath() -> [ProjectPickerWorkflowRequest] {
        let path = session.standardizedTypedPath
        guard session.typedPathState != .missing else {
            return [.askCreateDirectory(path: path)]
        }
        return [.confirmProjectPath(path: path, createIfMissing: false)]
    }

    private func reloadAfterInputChange(_ update: () -> Void) -> [ProjectPickerWorkflowRequest] {
        let previousInput = session.input
        update()
        guard session.input != previousInput else { return [] }
        scheduleDirectoryReload(pathState: session.pathState)
        return []
    }

    private func scheduleDirectoryReload(pathState: ProjectPickerPathState) {
        cancelDirectoryReload()
        let loadID = UUID()
        directoryLoadID = loadID

        if let cached = directoryCache[pathState.directoryPath] {
            let snapshot = session.pathService.snapshot(for: pathState, items: cached)
            applyDirectorySnapshot(snapshot, loadID: loadID)
            return
        }

        loadingMessageTask = Task { [weak self, loadingMessageDelay] in
            try? await Task.sleep(for: loadingMessageDelay)
            guard !Task.isCancelled else { return }
            self?.showLoadingMessage(loadID: loadID)
        }

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

    private func cancelDirectoryReload() {
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

    private static func itemsLoader(for pathService: ProjectPickerPathService) -> ProjectPickerDirectoryItemsLoader {
        { directoryPath in
            switch await pathService.directoryContents(atPath: directoryPath) {
            case let .success(items): items
            case .failure: nil
            }
        }
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
