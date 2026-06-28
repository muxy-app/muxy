import Foundation
import Testing

@testable import Muxy

@Suite("Project")
struct ProjectTests {
    @Test("Project decodes legacy records without worktree location")
    func projectLegacyDecodeDefaultsWorktreeLocation() throws {
        let json = """
        {
          "id": "\(UUID().uuidString)",
          "name": "Repo",
          "path": "/tmp/repo",
          "sortOrder": 0,
          "createdAt": "2024-01-01T00:00:00Z",
          "icon": null,
          "logo": null,
          "iconColor": null
        }
        """
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let project = try decoder.decode(Project.self, from: Data(json.utf8))

        #expect(project.preferredWorktreeParentPath == nil)
        #expect(!project.worktreesEnabled)
        #expect(!project.isPinned)
    }

    @Test("new projects default to worktrees disabled")
    func newProjectDisablesWorktrees() {
        let project = Project(name: "Repo", path: "/tmp/repo")

        #expect(!project.worktreesEnabled)
    }

    @Test("isPinned survives an encode/decode round-trip")
    func isPinnedRoundTrips() throws {
        var project = Project(name: "Repo", path: "/tmp/repo")
        project.isPinned = true

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let decoded = try decoder.decode(Project.self, from: encoder.encode(project))

        #expect(decoded.isPinned)
    }

    @Test("worktreesEnabled survives an encode/decode round-trip")
    func worktreesEnabledRoundTrips() throws {
        var project = Project(name: "Repo", path: "/tmp/repo")
        project.worktreesEnabled = true

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let decoded = try decoder.decode(Project.self, from: encoder.encode(project))

        #expect(decoded.worktreesEnabled)
    }

    @Test("home uses the reserved id, home name and expanded home path")
    func homeIdentity() {
        let home = Project.home

        #expect(home.id == Project.homeID)
        #expect(home.isHome)
        #expect(home.name == Project.homeName)
        #expect(home.path == FileManager.default.homeDirectoryForCurrentUser.path)
    }

    @Test("isHome is false for ordinary projects")
    func ordinaryProjectIsNotHome() {
        let project = Project(name: "Repo", path: "/tmp/repo")

        #expect(!project.isHome)
    }

    @Test("remoteDeviceID round-trips and marks the project remote")
    func remoteDeviceIDRoundTrips() throws {
        let project = Project(name: "api", path: "~/code/api", remoteDeviceID: UUID())

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let decoded = try decoder.decode(Project.self, from: encoder.encode(project))

        #expect(decoded.remoteDeviceID == project.remoteDeviceID)
        #expect(decoded.isRemote)
        #expect(!decoded.isHome)
    }

    @Test("legacy records without remoteDeviceID decode as local")
    func legacyRecordDecodesWithoutRemoteDeviceID() throws {
        let json = """
        {
          "id": "\(UUID().uuidString)",
          "name": "Repo",
          "path": "/tmp/repo",
          "sortOrder": 0,
          "createdAt": "2024-01-01T00:00:00Z"
        }
        """
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let project = try decoder.decode(Project.self, from: Data(json.utf8))

        #expect(project.remoteDeviceID == nil)
        #expect(!project.isRemote)
    }
}
