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
    func defaultLocationStatusReportsInvalidCustomPaths() throws {
        let suiteName = "ProjectPickerDefaultLocationStatusTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        let file = root.appendingPathComponent("file")
        try Data().write(to: file)
        let missing = root.appendingPathComponent("missing", isDirectory: true)

        ProjectPickerDefaultLocation.setCustomPath(from: root, defaults: defaults)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults) == .ready)

        ProjectPickerDefaultLocation.setCustomPath(from: file, defaults: defaults)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults) == .notDirectory)

        ProjectPickerDefaultLocation.setCustomPath(from: missing, defaults: defaults)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults) == .missing)
    }

    @Test("default location status reports unreadable custom paths")
    func defaultLocationStatusReportsUnreadableCustomPaths() throws {
        let suiteName = "ProjectPickerDefaultLocationUnreadableStatusTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: directory.path)
            try? FileManager.default.removeItem(at: directory)
        }

        try FileManager.default.setAttributes([.posixPermissions: 0o000], ofItemAtPath: directory.path)
        ProjectPickerDefaultLocation.setCustomPath(from: directory, defaults: defaults)

        #expect(ProjectPickerDefaultLocation.status(defaults: defaults) == .unreadable)
    }

    @Test("default location state includes display status warning and chooser fallback")
    func defaultLocationStateIncludesDisplayStatusWarningAndChooserFallback() throws {
        let suiteName = "ProjectPickerDefaultLocationStateTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }

        ProjectPickerDefaultLocation.setCustomPath(from: root, defaults: defaults)
        let readyState = ProjectPickerDefaultLocation.state(defaults: defaults)

        #expect(readyState.path == root.standardizedFileURL.path)
        #expect(readyState.displayPath == root.standardizedFileURL.path + "/")
        #expect(!readyState.usesAppDefault)
        #expect(readyState.status == .ready)
        #expect(readyState.warning == nil)
        #expect(readyState.chooserInitialPath == root.standardizedFileURL.path)

        let missing = root.appendingPathComponent("missing", isDirectory: true)
        ProjectPickerDefaultLocation.setCustomPath(from: missing, defaults: defaults)
        let missingState = ProjectPickerDefaultLocation.state(defaults: defaults)

        #expect(missingState.status == .missing)
        #expect(missingState.warning == "Default location no longer exists. Choose another folder or use the app default.")
        #expect(missingState.chooserInitialPath == NSHomeDirectory())
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
