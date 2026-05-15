import Foundation

struct ProjectPickerSession {
    private(set) var input: String
    private(set) var rows: [String] = []
    private(set) var highlightedIndex: Int?
    private(set) var directoryLoadState = ProjectPickerDirectoryLoadState.loading(showsMessage: false)

    let homeDirectory: String
    var projectPaths: [String]

    var navigator: ProjectPickerNavigator {
        ProjectPickerNavigator(input: input, homeDirectory: homeDirectory)
    }

    var highlightedRow: String? {
        guard let highlightedIndex, highlightedIndex < rows.count else { return nil }
        return rows[highlightedIndex]
    }

    var standardizedTypedPath: String {
        navigator.standardizedConfirmPath
    }

    var typedPathState: ProjectPickerTypedPathState {
        ProjectPickerNavigator.typedPathState(path: standardizedTypedPath)
    }

    var isExistingProject: Bool {
        projectPaths.contains(standardizedTypedPath)
    }

    var actionTitle: String {
        if isExistingProject { return "Open" }
        return typedPathState == .missing ? "Create & Add" : "Add"
    }

    var topRightActionTitle: String {
        if isExistingProject { return "Open Project" }
        return typedPathState == .missing ? "Create & Add Project" : "Add Project"
    }

    var ghostText: String {
        navigator.ghostText(highlightedRow: highlightedRow)
    }

    var projectRows: [String] {
        rows.filter { !isParentDirectoryRow($0) }
    }

    var hasParentRow: Bool {
        rows.contains { isParentDirectoryRow($0) }
    }

    var showsUnavailableProjectState: Bool {
        directoryLoadState.readFailed || projectRows.isEmpty
    }

    init(defaultDisplayPath: String, homeDirectory: String = NSHomeDirectory(), projectPaths: [String]) {
        input = defaultDisplayPath
        self.homeDirectory = homeDirectory
        self.projectPaths = projectPaths
    }

    mutating func setProjectPaths(_ projectPaths: [String]) {
        self.projectPaths = projectPaths
    }

    mutating func setInput(_ input: String) -> ProjectPickerEffect {
        self.input = input
        directoryLoadState = .loading(showsMessage: false)
        return .requestDirectoryReload(navigator)
    }

    mutating func showLoadingMessage() {
        guard directoryLoadState.isLoading else { return }
        directoryLoadState = .loading(showsMessage: true)
    }

    mutating func selectRow(at index: Int) {
        guard rows.indices.contains(index) else { return }
        highlightedIndex = index
    }

    mutating func applyDirectorySnapshot(_ snapshot: ProjectPickerDirectorySnapshot) {
        directoryLoadState = snapshot.readFailed ? .failed : .loaded
        rows = snapshot.rows
        highlightedIndex = initialHighlightedIndex(for: snapshot.rows)
    }

    mutating func handle(_ command: ProjectPickerCommand) -> [ProjectPickerEffect] {
        switch command {
        case .moveHighlightUp:
            moveHighlight(-1)
            return []
        case .moveHighlightDown:
            moveHighlight(1)
            return []
        case .openHighlighted:
            guard let highlightedRow else { return [] }
            return descend(highlightedRow)
        case .confirmTypedPath:
            return confirmTypedPath()
        case .goBack:
            return goUp()
        case .dismiss:
            return [.dismiss]
        case .completeHighlighted:
            guard let highlightedRow else { return [] }
            return [setInput(navigator.completedPath(highlightedRow: highlightedRow))]
        }
    }

    mutating func activate(row: String) -> [ProjectPickerEffect] {
        descend(row)
    }

    func confirmCreateDirectoryAccepted() -> [ProjectPickerEffect] {
        [.confirmProjectPath(path: standardizedTypedPath, createIfMissing: true)]
    }

    func confirmationFailurePresentation(
        for result: ProjectOpenConfirmationResult
    ) -> ProjectPickerConfirmationFailurePresentation {
        ProjectPickerConfirmationFailurePresentation(result: result, path: standardizedTypedPath)
    }

    func isParentDirectoryRow(_ row: String) -> Bool {
        navigator.isParentDirectoryRow(row)
    }

    private mutating func moveHighlight(_ delta: Int) {
        guard !rows.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : rows.count - 1
            return
        }
        highlightedIndex = max(0, min(rows.count - 1, current + delta))
    }

    private mutating func confirmTypedPath() -> [ProjectPickerEffect] {
        let shouldCreate = typedPathState == .missing
        if shouldCreate {
            return [.confirmCreateDirectory(path: standardizedTypedPath)]
        }
        return [.confirmProjectPath(path: standardizedTypedPath, createIfMissing: false)]
    }

    private mutating func descend(_ row: String) -> [ProjectPickerEffect] {
        if isParentDirectoryRow(row) {
            return goUp()
        }
        return [setInput(navigator.completedPath(highlightedRow: row))]
    }

    private mutating func goUp() -> [ProjectPickerEffect] {
        let parentPath = navigator.parentDisplayPath
        guard parentPath != input else { return [] }
        return [setInput(parentPath)]
    }

    private func initialHighlightedIndex(for rows: [String]) -> Int? {
        guard !rows.isEmpty else { return nil }
        guard rows.first.map(isParentDirectoryRow) == true, rows.count > 1 else { return 0 }
        return 1
    }
}

enum ProjectPickerEffect: Equatable {
    case requestDirectoryReload(ProjectPickerNavigator)
    case confirmCreateDirectory(path: String)
    case confirmProjectPath(path: String, createIfMissing: Bool)
    case chooseFinder
    case openSettingsFocusedOnDefaultLocation
    case dismiss
}

struct ProjectPickerConfirmationFailurePresentation: Equatable {
    let title: String
    let message: String

    init(result: ProjectOpenConfirmationResult, path: String) {
        switch result {
        case .notDirectory:
            title = "Path Is Not a Folder"
            message = "Muxy can only add folders as projects. Choose a folder or type a new folder path."
        case .missingDirectory:
            title = "Could Not Add Project"
            message = "Muxy couldn't find \"\(path)\". Check the path and try again."
        case .createFailed:
            title = "Could Not Create Project Folder"
            message = "Muxy couldn't create and add \"\(path)\". Check that you have permission to use this location."
        default:
            title = "Could Not Add Project"
            message = "Muxy couldn't add \"\(path)\". Check that the folder exists and you have permission to use it."
        }
    }
}

enum ProjectPickerDirectoryLoadState: Equatable {
    case loading(showsMessage: Bool)
    case loaded
    case failed

    var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }

    var showsMessage: Bool {
        if case let .loading(showsMessage) = self { return showsMessage }
        return false
    }

    var readFailed: Bool {
        self == .failed
    }
}
