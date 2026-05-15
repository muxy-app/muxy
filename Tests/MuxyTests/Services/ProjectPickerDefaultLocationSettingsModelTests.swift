import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerDefaultLocationSettingsModel")
@MainActor
struct ProjectPickerDefaultLocationSettingsModelTests {
    @Test("initial state is loaded from the default location model")
    func initialStateLoadsFromDefaultLocationModel() {
        let context = TestContext()
        defer { context.cleanup() }

        let model = context.model()

        #expect(model.state.path == "/Users/alice")
        #expect(model.state.displayPath == "~/")
        #expect(model.state.status == .ready)
        #expect(model.state.usesAppDefault)
        #expect(model.isResetDisabled)
    }

    @Test("reset clears custom storage and refreshes state")
    func resetClearsCustomStorageAndRefreshesState() {
        let context = TestContext()
        defer { context.cleanup() }
        ProjectPickerDefaultLocation.setCustomPath("~/Projects", defaults: context.defaults)
        let model = context.model()

        model.reset()

        #expect(model.state.path == "/Users/alice")
        #expect(model.state.displayPath == "~/")
        #expect(model.state.usesAppDefault)
        #expect(model.isResetDisabled)
    }

    @Test("chosen folder stores standardized path and refreshes state")
    func chosenFolderStoresStandardizedPathAndRefreshesState() {
        let context = TestContext()
        defer { context.cleanup() }
        context.panel.selectedURL = URL(fileURLWithPath: "/Users/alice/Picked/.", isDirectory: true)
        let model = context.model()

        model.chooseFolder()

        #expect(context.panel.requests == [
            ProjectPickerDefaultLocationPanelRequest(
                initialPath: "/Users/alice",
                message: "Select the default location for the project picker"
            ),
        ])
        #expect(ProjectPickerDefaultLocation.path(defaults: context.defaults, pathService: context.pathService) == "/Users/alice/Picked")
        #expect(model.state.path == "/Users/alice/Picked")
        #expect(model.state.displayPath == "~/Picked/")
        #expect(!model.state.usesAppDefault)
        #expect(!model.isResetDisabled)
    }

    @Test("ready custom path is used as the chooser initial path")
    func readyCustomPathIsChooserInitialPath() {
        let context = TestContext()
        defer { context.cleanup() }
        ProjectPickerDefaultLocation.setCustomPath("~/Projects", defaults: context.defaults)
        let model = context.model()

        model.chooseFolder()

        #expect(context.panel.requests.first?.initialPath == "/Users/alice/Projects")
    }

    @Test("invalid custom path falls back to home for the chooser initial path")
    func invalidCustomPathFallsBackToHomeForChooserInitialPath() {
        let context = TestContext()
        defer { context.cleanup() }
        ProjectPickerDefaultLocation.setCustomPath("~/Missing", defaults: context.defaults)
        let model = context.model()

        model.chooseFolder()

        #expect(context.panel.requests.first?.initialPath == "/Users/alice")
        #expect(model.state.status == .missing)
        #expect(model.state.warning == "Default location no longer exists. Choose another folder or use the app default.")
    }

    @Test("cancel leaves state and storage unchanged")
    func cancelLeavesStateAndStorageUnchanged() {
        let context = TestContext()
        defer { context.cleanup() }
        ProjectPickerDefaultLocation.setCustomPath("~/Projects", defaults: context.defaults)
        let model = context.model()
        context.panel.selectedURL = nil

        model.chooseFolder()

        #expect(ProjectPickerDefaultLocation.path(defaults: context.defaults, pathService: context.pathService) == "/Users/alice/Projects")
        #expect(model.state.path == "/Users/alice/Projects")
        #expect(!model.state.usesAppDefault)
    }

    @Test("app activation refreshes warning state")
    func appActivationRefreshesWarningState() {
        let context = TestContext()
        defer { context.cleanup() }
        ProjectPickerDefaultLocation.setCustomPath("~/Projects", defaults: context.defaults)
        let model = context.model()

        context.fileSystem.directoryStates["/Users/alice/Projects"] = .missing
        context.fileSystem.readablePaths.remove("/Users/alice/Projects")
        model.handleAppActivation()

        #expect(model.state.status == .missing)
        #expect(model.state.warning == "Default location no longer exists. Choose another folder or use the app default.")
    }

    @Test("focus request is consumed once")
    func focusRequestIsConsumedOnce() {
        let context = TestContext()
        defer { context.cleanup() }
        let model = context.model()

        context.focusCoordinator.request(.projectPickerDefaultLocation)
        model.consumeFocusRequest()
        model.consumeFocusRequest()

        #expect(model.focusRequestID == 1)
    }
}

@MainActor
private final class TestContext {
    let suiteName: String
    let defaults: UserDefaults
    let fileSystem = ProjectPickerDefaultLocationSettingsFileSystemFake()
    let panel = ProjectPickerDefaultLocationPanelFake()
    let focusCoordinator = SettingsFocusCoordinator(notificationCenter: NotificationCenter())
    let pathService: ProjectPickerPathService

    init() {
        suiteName = "ProjectPickerDefaultLocationSettingsModelTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            preconditionFailure("Unable to create isolated UserDefaults suite")
        }
        self.defaults = defaults
        pathService = ProjectPickerPathService(homeDirectory: "/Users/alice", fileSystem: fileSystem)
        fileSystem.directoryStates = [
            "/Users/alice": .directory,
            "/Users/alice/Projects": .directory,
            "/Users/alice/Picked": .directory,
        ]
        fileSystem.readablePaths = [
            "/Users/alice",
            "/Users/alice/Projects",
            "/Users/alice/Picked",
        ]
    }

    func cleanup() {
        defaults.removePersistentDomain(forName: suiteName)
    }

    func model() -> ProjectPickerDefaultLocationSettingsModel {
        ProjectPickerDefaultLocationSettingsModel(
            defaults: defaults,
            pathService: pathService,
            panel: panel,
            focusCoordinator: focusCoordinator
        )
    }
}

private struct ProjectPickerDefaultLocationPanelRequest: Equatable {
    let initialPath: String
    let message: String
}

@MainActor
private final class ProjectPickerDefaultLocationPanelFake: ProjectPickerDefaultLocationPanel {
    var requests: [ProjectPickerDefaultLocationPanelRequest] = []
    var selectedURL: URL?

    func selectDirectory(initialPath: String, message: String) -> URL? {
        requests.append(ProjectPickerDefaultLocationPanelRequest(initialPath: initialPath, message: message))
        return selectedURL
    }
}

private final class ProjectPickerDefaultLocationSettingsFileSystemFake: ProjectPickerFileSystem {
    var directoryStates: [String: ProjectPickerFileSystemDirectoryState] = [:]
    var readablePaths: Set<String> = []

    func directoryState(atPath path: String) -> ProjectPickerFileSystemDirectoryState {
        directoryStates[path] ?? .missing
    }

    func isReadableFile(atPath path: String) -> Bool {
        readablePaths.contains(path)
    }

    func contentsOfDirectory(atPath path: String) throws -> [ProjectPickerFileSystemDirectoryEntry] {
        []
    }
}
