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

        defaults.set("~/Projects", forKey: ProjectPickerDefaultLocation.storageKey)

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

        defaults.set(root.path, forKey: ProjectPickerDefaultLocation.storageKey)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults) == .ready)

        defaults.set(file.path, forKey: ProjectPickerDefaultLocation.storageKey)
        #expect(ProjectPickerDefaultLocation.status(defaults: defaults) == .notDirectory)

        defaults.set(missing.path, forKey: ProjectPickerDefaultLocation.storageKey)
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
        defaults.set(directory.path, forKey: ProjectPickerDefaultLocation.storageKey)

        #expect(ProjectPickerDefaultLocation.status(defaults: defaults) == .unreadable)
    }
}
