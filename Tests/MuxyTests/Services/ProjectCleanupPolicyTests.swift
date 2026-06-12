import Foundation
import Testing

@testable import Muxy

@Suite("ProjectCleanupPolicy")
struct ProjectCleanupPolicyTests {
    @Test("local project is removed when workspace becomes empty")
    func localProjectIsRemoved() {
        let project = Project(name: "muxy", path: "/tmp/muxy")

        #expect(ProjectCleanupPolicy.shouldRemoveStoredProjectWhenWorkspaceEmptied(project))
    }

    @Test("device-backed remote project stays stored when workspace becomes empty")
    func remoteDeviceProjectIsPreserved() {
        let project = Project(
            name: "muxy",
            path: "~/Documents/workspace/muxy",
            remoteDeviceID: UUID()
        )

        #expect(!ProjectCleanupPolicy.shouldRemoveStoredProjectWhenWorkspaceEmptied(project))
    }
}
