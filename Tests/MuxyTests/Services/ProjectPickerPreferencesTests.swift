import Foundation
import Testing

@testable import Muxy

@Suite("ProjectPickerPreferences")
struct ProjectPickerPreferencesTests {
    @Test("custom picker is the default and the selected picker persists")
    func pickerModePersists() throws {
        let suiteName = "ProjectPickerPreferencesTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let preferences = ProjectPickerPreferences(defaults: defaults)

        #expect(preferences.mode == .custom)

        preferences.mode = .finder

        #expect(ProjectPickerPreferences(defaults: defaults).mode == .finder)
    }

    @Test("default location defaults to home and supports a custom path")
    func defaultLocationPersists() throws {
        let suiteName = "ProjectPickerDefaultLocationTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        #expect(ProjectPickerDefaultLocation.path(defaults: defaults) == NSHomeDirectory())
        #expect(ProjectPickerDefaultLocation.displayPath(defaults: defaults) == "~/")
        #expect(ProjectPickerDefaultLocation.usesAppDefault(defaults: defaults))

        ProjectPickerDefaultLocation.setCustomPath("~/Projects", defaults: defaults)

        #expect(ProjectPickerDefaultLocation.path(defaults: defaults) == NSHomeDirectory() + "/Projects")
        #expect(ProjectPickerDefaultLocation.displayPath(defaults: defaults) == "~/Projects/")
        #expect(ProjectPickerDefaultLocation.displayPath(storedCustomPath: "~/Projects") == "~/Projects/")
        #expect(ProjectPickerDefaultLocation.displayPath(storedCustomPath: "") == "~/")
        #expect(!ProjectPickerDefaultLocation.usesAppDefault(defaults: defaults))
    }

    @Test("default location status reports invalid custom paths")
    func defaultLocationStatusReportsInvalidCustomPaths() {
        let suiteName = "ProjectPickerDefaultLocationStatusTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let pathService = ProjectPickerPathService(fileSystem: ProjectPickerDefaultLocationFileSystemStub(
            directoryStates: [
                "/tmp/ready": .directory,
                "/tmp/file": .notDirectory,
            ],
            readablePaths: ["/tmp/ready"]
        ))

        ProjectPickerDefaultLocation.setCustomPath("/tmp/ready", defaults: defaults)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults, pathService: pathService) == .ready)

        ProjectPickerDefaultLocation.setCustomPath("/tmp/file", defaults: defaults)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults, pathService: pathService) == .notDirectory)

        ProjectPickerDefaultLocation.setCustomPath("/tmp/missing", defaults: defaults)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults, pathService: pathService) == .missing)
    }

    @Test("default location status reports unreadable custom paths")
    func defaultLocationStatusReportsUnreadableCustomPaths() {
        let suiteName = "ProjectPickerDefaultLocationUnreadableStatusTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let pathService = ProjectPickerPathService(fileSystem: ProjectPickerDefaultLocationFileSystemStub(
            directoryStates: ["/tmp/unreadable": .directory]
        ))

        ProjectPickerDefaultLocation.setCustomPath("/tmp/unreadable", defaults: defaults)

        #expect(ProjectPickerDefaultLocation.status(defaults: defaults, pathService: pathService) == .unreadable)
    }

    @Test("default location state includes display status warning and chooser fallback")
    func defaultLocationStateIncludesDisplayStatusWarningAndChooserFallback() {
        let suiteName = "ProjectPickerDefaultLocationStateTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }
        let pathService = ProjectPickerPathService(
            homeDirectory: "/Users/alice",
            fileSystem: ProjectPickerDefaultLocationFileSystemStub(
                directoryStates: ["/Users/alice/Projects": .directory],
                readablePaths: ["/Users/alice/Projects"]
            )
        )

        ProjectPickerDefaultLocation.setCustomPath("~/Projects", defaults: defaults)
        let readyState = ProjectPickerDefaultLocation.state(defaults: defaults, pathService: pathService)

        #expect(readyState.path == "/Users/alice/Projects")
        #expect(readyState.displayPath == "~/Projects/")
        #expect(!readyState.usesAppDefault)
        #expect(readyState.status == .ready)
        #expect(readyState.warning == nil)
        #expect(readyState.chooserInitialPath == "/Users/alice/Projects")

        ProjectPickerDefaultLocation.setCustomPath("~/Missing", defaults: defaults)
        let missingState = ProjectPickerDefaultLocation.state(defaults: defaults, pathService: pathService)

        #expect(missingState.status == .missing)
        #expect(missingState.warning == "Default location no longer exists. Choose another folder or use the app default.")
        #expect(missingState.chooserInitialPath == "/Users/alice")
    }

    @Test("default location resets and normalizes selected directories through the model")
    func defaultLocationResetsAndNormalizesSelectedDirectoriesThroughModel() throws {
        let suiteName = "ProjectPickerDefaultLocationMutationTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        let nested = root.appendingPathComponent("nested", isDirectory: true)
        try FileManager.default.createDirectory(at: nested, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let selectedURL = URL(fileURLWithPath: nested.path + "/.", isDirectory: true)
        ProjectPickerDefaultLocation.setCustomPath(from: selectedURL, defaults: defaults)

        #expect(ProjectPickerDefaultLocation.path(defaults: defaults) == nested.standardizedFileURL.path)
        #expect(!ProjectPickerDefaultLocation.state(defaults: defaults).usesAppDefault)

        ProjectPickerDefaultLocation.resetToAppDefault(defaults: defaults)

        #expect(ProjectPickerDefaultLocation.path(defaults: defaults) == NSHomeDirectory())
        #expect(ProjectPickerDefaultLocation.state(defaults: defaults).usesAppDefault)
    }
}

private struct ProjectPickerDefaultLocationFileSystemStub: ProjectPickerFileSystem {
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
