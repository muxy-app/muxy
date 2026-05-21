import Foundation

struct ProjectPickerSession {
    private(set) var input: String
    private(set) var rows: [ProjectPickerDirectoryItem] = []
    private(set) var highlightedIndex: Int?
    private(set) var directoryLoadState = ProjectPickerDirectoryLoadState.loading(showsMessage: false)

    let homeDirectory: String
    let pathService: ProjectPickerPathService
    var projectPaths: [String]

    var pathState: ProjectPickerPathState {
        pathService.state(for: input)
    }

    var navigator: ProjectPickerNavigator {
        ProjectPickerNavigator(pathState: pathState)
    }

    var highlightedItem: ProjectPickerDirectoryItem? {
        guard let highlightedIndex, highlightedIndex < rows.count else { return nil }
        return rows[highlightedIndex]
    }

    var highlightedRow: String? {
        highlightedItem?.name
    }

    var standardizedTypedPath: String {
        pathState.standardizedConfirmPath
    }

    var typedPathState: ProjectPickerTypedPathState {
        pathService.typedPathState(path: standardizedTypedPath)
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

    var projectRows: [ProjectPickerDirectoryItem] {
        rows.filter { !$0.isParent }
    }

    var hasParentRow: Bool {
        rows.contains(where: \.isParent)
    }

    var showsUnavailableProjectState: Bool {
        directoryLoadState.readFailed || projectRows.isEmpty
    }

    init(
        defaultDisplayPath: String,
        homeDirectory: String = NSHomeDirectory(),
        projectPaths: [String],
        pathService: ProjectPickerPathService? = nil
    ) {
        input = defaultDisplayPath
        self.homeDirectory = homeDirectory
        self.projectPaths = projectPaths
        self.pathService = pathService ?? ProjectPickerPathService(homeDirectory: homeDirectory)
    }

    mutating func setProjectPaths(_ projectPaths: [String]) {
        self.projectPaths = projectPaths
    }

    mutating func setInput(_ input: String) {
        self.input = input
        directoryLoadState = .loading(showsMessage: false)
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

    mutating func handle(_ command: ProjectPickerCommand) {
        switch command {
        case .moveHighlightUp:
            moveHighlight(-1)
        case .moveHighlightDown:
            moveHighlight(1)
        case .openHighlighted:
            guard let highlightedItem else { return }
            descend(highlightedItem)
        case .confirmTypedPath:
            return
        case .goBack:
            goUp()
        case .dismiss:
            return
        case .completeHighlighted:
            guard let highlightedRow else { return }
            setInput(navigator.completedPath(highlightedRow: highlightedRow))
        }
    }

    mutating func activate(row: ProjectPickerDirectoryItem) {
        descend(row)
    }

    func isParentDirectoryRow(_ row: String) -> Bool {
        navigator.isParentDirectoryRow(row)
    }

    func isParentDirectoryRow(_ row: ProjectPickerDirectoryItem) -> Bool {
        row.isParent
    }

    private mutating func moveHighlight(_ delta: Int) {
        guard !rows.isEmpty else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : rows.count - 1
            return
        }
        highlightedIndex = max(0, min(rows.count - 1, current + delta))
    }

    private mutating func descend(_ row: ProjectPickerDirectoryItem) {
        if row.isParent {
            goUp()
            return
        }
        setInput(navigator.completedPath(highlightedRow: row.name))
    }

    private mutating func goUp() {
        let parentPath = navigator.parentDisplayPath
        guard parentPath != input else { return }
        setInput(parentPath)
    }

    private func initialHighlightedIndex(for rows: [ProjectPickerDirectoryItem]) -> Int? {
        guard !rows.isEmpty else { return nil }
        guard rows.first?.isParent == true, rows.count > 1 else { return 0 }
        return 1
    }
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
