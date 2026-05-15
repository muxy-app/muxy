import Foundation

typealias ProjectPickerDirectoryLoader = @Sendable (ProjectPickerNavigator) async -> ProjectPickerDirectorySnapshot

@MainActor
@Observable
final class ProjectPickerWorkflow {
    private(set) var session: ProjectPickerSession

    @ObservationIgnored private var directoryLoadID = UUID()
    @ObservationIgnored private var reloadTask: Task<Void, Never>?
    @ObservationIgnored private var loadingMessageTask: Task<Void, Never>?
    @ObservationIgnored private let directoryLoader: ProjectPickerDirectoryLoader
    @ObservationIgnored private let reloadDelay: Duration
    @ObservationIgnored private let loadingMessageDelay: Duration
    @ObservationIgnored private var didAppear = false

    init(
        defaultDisplayPath: String = ProjectPickerDefaultLocation.state.displayPath,
        homeDirectory: String = NSHomeDirectory(),
        projectPaths: [String],
        directoryLoader: @escaping ProjectPickerDirectoryLoader = ProjectPickerWorkflow.liveDirectoryLoader,
        reloadDelay: Duration = .milliseconds(100),
        loadingMessageDelay: Duration = .milliseconds(500)
    ) {
        session = ProjectPickerSession(
            defaultDisplayPath: defaultDisplayPath,
            homeDirectory: homeDirectory,
            projectPaths: projectPaths
        )
        self.directoryLoader = directoryLoader
        self.reloadDelay = reloadDelay
        self.loadingMessageDelay = loadingMessageDelay
    }

    func appear() {
        guard !didAppear else { return }
        didAppear = true
        scheduleDirectoryReload(navigator: session.navigator)
    }

    func cancel() {
        cancelDirectoryReload()
    }

    func setProjectPaths(_ projectPaths: [String]) {
        session.setProjectPaths(projectPaths)
    }

    func setInput(_ input: String) -> [ProjectPickerWorkflowRequest] {
        execute(session.setInput(input))
    }

    func selectRow(at index: Int) {
        session.selectRow(at: index)
    }

    func activate(row: String) -> [ProjectPickerWorkflowRequest] {
        execute(session.activate(row: row))
    }

    func handle(_ command: ProjectPickerCommand) -> [ProjectPickerWorkflowRequest] {
        execute(session.handle(command))
    }

    func chooseWithFinder() -> [ProjectPickerWorkflowRequest] {
        execute([.dismiss, .chooseFinder])
    }

    func editDefaultLocation() -> [ProjectPickerWorkflowRequest] {
        execute([.dismiss, .openSettingsFocusedOnDefaultLocation])
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

    private func execute(_ effect: ProjectPickerEffect) -> [ProjectPickerWorkflowRequest] {
        execute([effect])
    }

    private func execute(_ effects: [ProjectPickerEffect]) -> [ProjectPickerWorkflowRequest] {
        effects.flatMap(executeSingle)
    }

    private func executeSingle(_ effect: ProjectPickerEffect) -> [ProjectPickerWorkflowRequest] {
        switch effect {
        case let .requestDirectoryReload(navigator):
            scheduleDirectoryReload(navigator: navigator)
            return []
        case let .confirmCreateDirectory(path):
            return [.askCreateDirectory(path: path)]
        case let .confirmProjectPath(path, createIfMissing):
            return [.confirmProjectPath(path: path, createIfMissing: createIfMissing)]
        case .chooseFinder:
            return [.chooseFinder]
        case .openSettingsFocusedOnDefaultLocation:
            return [.openSettingsFocusedOnDefaultLocation]
        case .dismiss:
            return [.dismiss]
        }
    }

    private func scheduleDirectoryReload(navigator: ProjectPickerNavigator) {
        cancelDirectoryReload()
        let loadID = UUID()
        directoryLoadID = loadID

        loadingMessageTask = Task { [weak self, loadingMessageDelay] in
            try? await Task.sleep(for: loadingMessageDelay)
            guard !Task.isCancelled else { return }
            self?.showLoadingMessage(loadID: loadID)
        }

        reloadTask = Task { [weak self, reloadDelay, directoryLoader] in
            try? await Task.sleep(for: reloadDelay)
            guard !Task.isCancelled else { return }
            let snapshot = await directoryLoader(navigator)
            guard !Task.isCancelled else { return }
            self?.applyDirectorySnapshot(snapshot, loadID: loadID)
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

    private static let liveDirectoryLoader: ProjectPickerDirectoryLoader = { navigator in
        await Task.detached(priority: .userInitiated) {
            ProjectPickerDirectorySnapshot.load(navigator: navigator)
        }.value
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
