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

    @Test("default directory defaults to home and supports a custom path")
    func defaultDirectoryPersists() throws {
        let suiteName = "ProjectPickerDefaultDirectoryTests-\(UUID().uuidString)"
        guard let defaults = UserDefaults(suiteName: suiteName) else {
            Issue.record("Unable to create isolated UserDefaults suite")
            return
        }
        defer { defaults.removePersistentDomain(forName: suiteName) }

        #expect(ProjectPickerDefaultDirectory.path(defaults: defaults) == NSHomeDirectory())
        #expect(ProjectPickerDefaultDirectory.displayPath(defaults: defaults) == "~/")
        #expect(ProjectPickerDefaultDirectory.usesAppDefault(defaults: defaults))

        defaults.set("~/Projects", forKey: ProjectPickerDefaultDirectory.storageKey)

        #expect(ProjectPickerDefaultDirectory.path(defaults: defaults) == NSHomeDirectory() + "/Projects")
        #expect(ProjectPickerDefaultDirectory.displayPath(defaults: defaults) == "~/Projects/")
        #expect(!ProjectPickerDefaultDirectory.usesAppDefault(defaults: defaults))
    }

    @Test("default directory status reports invalid custom paths")
    func defaultDirectoryStatusReportsInvalidCustomPaths() throws {
        let suiteName = "ProjectPickerDefaultDirectoryStatusTests-\(UUID().uuidString)"
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

        defaults.set(root.path, forKey: ProjectPickerDefaultDirectory.storageKey)
        #expect(ProjectPickerDefaultDirectory.status(defaults: defaults) == .ready)

        defaults.set(file.path, forKey: ProjectPickerDefaultDirectory.storageKey)
        #expect(ProjectPickerDefaultDirectory.status(defaults: defaults) == .notDirectory)

        defaults.set(missing.path, forKey: ProjectPickerDefaultDirectory.storageKey)
        #expect(ProjectPickerDefaultDirectory.status(defaults: defaults) == .missing)
    }
}
