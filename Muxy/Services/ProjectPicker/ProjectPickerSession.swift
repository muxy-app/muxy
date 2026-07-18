import Foundation

enum ProjectPickerInputMode: Equatable {
    case folderSearch
    case path

    static func resolve(input: String, allowsFolderSearch: Bool) -> ProjectPickerInputMode {
        guard allowsFolderSearch else { return .path }
        let value = input.trimmingCharacters(in: .whitespacesAndNewlines)
        let isExplicitPath = value.hasPrefix("/")
            || value == "~"
            || value.hasPrefix("~/")
            || value == "."
            || value.hasPrefix("./")
            || value == ".."
            || value.hasPrefix("../")
        guard !isExplicitPath else { return .path }
        return .folderSearch
    }
}

struct ProjectPickerSession {
    private(set) var input: String
    private(set) var searchResults: [ProjectPickerFolderSearchResult] = []
    private(set) var folderSearchIsTruncated = false
    private(set) var folderSearchHasMoreResults = false
    private(set) var rows: [ProjectPickerDirectoryItem] = []
    private(set) var highlightedIndex: Int?
    private(set) var directoryLoadState = ProjectPickerDirectoryLoadState.loading(showsMessage: false)

    let homeDirectory: String
    let pathService: ProjectPickerPathService
    let searchRootPath: String
    let allowsFolderSearch: Bool
    var projectPaths: [String]

    var inputMode: ProjectPickerInputMode {
        ProjectPickerInputMode.resolve(input: input, allowsFolderSearch: allowsFolderSearch)
    }

    var searchQuery: String {
        input.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    var pathState: ProjectPickerPathState {
        pathService.state(for: input)
    }

    var navigator: ProjectPickerNavigator {
        ProjectPickerNavigator(pathState: pathState)
    }

    var highlightedItem: ProjectPickerDirectoryItem? {
        guard inputMode == .path else { return nil }
        guard let highlightedIndex, highlightedIndex < rows.count else { return nil }
        return rows[highlightedIndex]
    }

    var highlightedSearchResult: ProjectPickerFolderSearchResult? {
        guard inputMode == .folderSearch else { return nil }
        guard let highlightedIndex, highlightedIndex < searchResults.count else { return nil }
        return searchResults[highlightedIndex]
    }

    var highlightedRow: String? {
        highlightedItem?.name
    }

    var standardizedTypedPath: String {
        pathState.standardizedConfirmPath
    }

    var confirmationPath: String? {
        switch inputMode {
        case .folderSearch:
            highlightedSearchResult?.path
        case .path:
            standardizedTypedPath
        }
    }

    var typedPathState: ProjectPickerTypedPathState {
        pathService.typedPathState(path: standardizedTypedPath)
    }

    var isExistingProject: Bool {
        guard let confirmationPath else { return false }
        return projectPaths.contains { pathService.standardize($0) == confirmationPath }
    }

    var actionTitle: String {
        if isExistingProject { return "Open" }
        if inputMode == .folderSearch { return "Add" }
        return typedPathState == .missing ? "Create & Add" : "Add"
    }

    var topRightActionTitle: String {
        if isExistingProject { return "Open Project" }
        if inputMode == .folderSearch { return "Add Project" }
        return typedPathState == .missing ? "Create & Add Project" : "Add Project"
    }

    var ghostText: String {
        guard inputMode == .path else { return "" }
        return navigator.ghostText(highlightedRow: highlightedRow)
    }

    var projectRows: [ProjectPickerDirectoryItem] {
        rows.filter { !$0.isParent }
    }

    var hasParentRow: Bool {
        guard inputMode == .path else { return false }
        return rows.contains(where: \.isParent)
    }

    var showsUnavailableProjectState: Bool {
        switch inputMode {
        case .folderSearch:
            searchResults.isEmpty
        case .path:
            directoryLoadState.readFailed || projectRows.isEmpty
        }
    }

    init(
        defaultDisplayPath: String,
        homeDirectory: String = NSHomeDirectory(),
        projectPaths: [String],
        pathService: ProjectPickerPathService? = nil,
        allowsFolderSearch: Bool = true
    ) {
        let pathService = pathService ?? ProjectPickerPathService(homeDirectory: homeDirectory)
        input = allowsFolderSearch ? "" : defaultDisplayPath
        self.homeDirectory = homeDirectory
        self.projectPaths = projectPaths
        self.pathService = pathService
        searchRootPath = pathService.standardize(pathService.expandedPath(defaultDisplayPath))
        self.allowsFolderSearch = allowsFolderSearch
    }

    init(
        projectPaths: [String],
        context: WorkspaceContext
    ) {
        guard case let .ssh(destination) = context else {
            self.init(
                defaultDisplayPath: ProjectPickerDefaultLocation.state.displayPath,
                projectPaths: projectPaths
            )
            return
        }
        let remoteHome = destination.remoteRoot
        let displayPath = remoteHome.hasSuffix("/") ? remoteHome : remoteHome + "/"
        let service = ProjectPickerPathService(
            homeDirectory: remoteHome,
            fileSystem: RemoteProjectPickerFileSystem(destination: destination),
            isRemote: true
        )
        self.init(
            defaultDisplayPath: displayPath,
            homeDirectory: remoteHome,
            projectPaths: projectPaths,
            pathService: service,
            allowsFolderSearch: false
        )
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
        guard (0 ..< activeRowCount).contains(index) else { return }
        highlightedIndex = index
    }

    mutating func applyFolderSearchSnapshot(_ snapshot: ProjectPickerFolderSearchSnapshot) {
        directoryLoadState = snapshot.readFailed ? .failed : .loaded
        searchResults = snapshot.results
        folderSearchIsTruncated = snapshot.isTruncated
        folderSearchHasMoreResults = snapshot.hasMoreResults
        highlightedIndex = searchResults.isEmpty ? nil : 0
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
            guard inputMode == .path else { return }
            guard let highlightedItem else { return }
            descend(highlightedItem)
        case .confirmTypedPath:
            return
        case .goBack:
            guard inputMode == .path else { return }
            goUp()
        case .dismiss:
            return
        case .completeHighlighted:
            if let highlightedSearchResult {
                setInput(pathService.directoryDisplayPath(highlightedSearchResult.path))
                return
            }
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
        guard activeRowCount > 0 else { return }
        guard let current = highlightedIndex else {
            highlightedIndex = delta > 0 ? 0 : activeRowCount - 1
            return
        }
        highlightedIndex = max(0, min(activeRowCount - 1, current + delta))
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

    private var activeRowCount: Int {
        switch inputMode {
        case .folderSearch:
            searchResults.count
        case .path:
            rows.count
        }
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
