import Foundation
import Testing

@testable import Muxy

@Suite("ProjectGroup decoding and migration")
struct ProjectGroupMigrationTests {
    private func decode(_ json: String) throws -> ProjectGroup {
        try JSONDecoder().decode(ProjectGroup.self, from: Data(json.utf8))
    }

    @Test("legacy rows backfill to local with no remote data")
    func legacyBackfill() throws {
        let json = """
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "Personal",
          "sortOrder": 2,
          "projectIDs": ["00000000-0000-0000-0000-0000000000AA"]
        }
        """
        let group = try decode(json)
        #expect(group.type == .local)
        #expect(group.remoteDeviceID == nil)
        #expect(group.legacySSHData == nil)
        #expect(group.remoteProjects.isEmpty)
        #expect(group.workspaceContext(device: nil) == .local)
    }

    @Test("device-backed ssh rows round-trip without legacy data")
    func sshRoundTrip() throws {
        let original = ProjectGroup(
            name: "prod",
            sortOrder: 1,
            type: .ssh,
            remoteDeviceID: UUID(),
            remoteProjects: [RemoteProject(name: "api", path: "~/code/api")]
        )
        let data = try JSONEncoder().encode(original)
        let decoded = try JSONDecoder().decode(ProjectGroup.self, from: data)
        #expect(decoded.type == .ssh)
        #expect(decoded.remoteDeviceID == original.remoteDeviceID)
        #expect(decoded.legacySSHData == nil)
        #expect(decoded.remoteProjects.first?.path == "~/code/api")
    }

    @Test("legacy ssh rows decode their inline sshData for migration")
    func legacySSHDecodes() throws {
        let json = """
        {
          "id": "00000000-0000-0000-0000-000000000002",
          "name": "prod",
          "sortOrder": 1,
          "type": "ssh",
          "sshData": { "host": "prod", "remoteRoot": "~/code" }
        }
        """
        let group = try decode(json)
        #expect(group.type == .ssh)
        #expect(group.remoteDeviceID == nil)
        #expect(group.legacySSHData?.host == "prod")
        #expect(group.legacySSHData?.remoteRoot == "~/code")
        #expect(group.legacySSHData?.environment == SSHEnvironmentVariables.default)
    }

    @Test("empty remote root defaults to home")
    func emptyRootDefaults() {
        let data = SSHWorkspaceData(host: "prod", remoteRoot: "  ")
        #expect(data.remoteRoot == "~")
    }

    @Test("ssh workspace exposes a remote home project at the device root")
    func remoteHomeProject() {
        let device = RemoteDevice(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: "~/code"))
        let group = ProjectGroup(name: "prod", type: .ssh, remoteDeviceID: device.id)
        let home = group.remoteHomeProject(device: device)
        #expect(home?.path == "~/code")
        #expect(home?.isRemote == true)
        #expect(home?.isHome == true)
        #expect(home?.remoteWorkspaceID == group.id)
        #expect(home?.id == ProjectGroup.remoteHomeID(for: group.id))
        #expect(home?.id != group.id)
    }

    @Test("local workspace has no remote home project")
    func localHasNoRemoteHome() {
        let group = ProjectGroup(name: "Personal")
        #expect(group.remoteHomeProject(device: nil) == nil)
    }
}
