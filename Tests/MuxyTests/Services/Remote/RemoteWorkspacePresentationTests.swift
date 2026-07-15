import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("Remote workspace presentation")
@MainActor
struct RemoteWorkspacePresentationTests {
    @Test("remote identities are stable, reversible, and scoped by entity")
    func identityMapping() {
        let identities = RemoteWorkspaceIdentityMap()
        let remoteID = UUID()

        let projectID = identities.presentationID(for: remoteID, entity: .project)
        let repeatedProjectID = identities.presentationID(for: remoteID, entity: .project)
        let paneID = identities.presentationID(for: remoteID, entity: .pane)

        #expect(projectID == repeatedProjectID)
        #expect(projectID != paneID)
        #expect(projectID != remoteID)
        #expect(identities.remoteID(for: projectID) == remoteID)
        #expect(identities.remoteID(for: paneID) == remoteID)
    }

    @Test("workspace DTOs hydrate shared models without local terminal backends")
    func sharedModelHydration() {
        let identities = RemoteWorkspaceIdentityMap()
        let projectID = UUID()
        let worktreeID = UUID()
        let paneID = UUID()
        let terminal = TabDTO(
            id: UUID(),
            kind: .terminal,
            title: "Shell",
            isPinned: false,
            paneID: paneID
        )
        let browser = TabDTO(
            id: UUID(),
            kind: .browser,
            title: "Docs",
            isPinned: true
        )
        let area = TabAreaDTO(
            id: UUID(),
            projectPath: "/repo",
            tabs: [terminal, browser],
            activeTabID: terminal.id
        )
        let workspace = WorkspaceDTO(
            projectID: projectID,
            worktreeID: worktreeID,
            focusedAreaID: area.id,
            root: .tabArea(area)
        )

        let presentation = RemoteWorkspacePresentationBuilder.workspace(
            from: workspace,
            identities: identities
        )
        let presentedArea = presentation.focusedArea

        #expect(presentation.projectID != projectID)
        #expect(presentation.worktreeID != worktreeID)
        #expect(presentedArea?.tabs.count == 2)
        #expect(presentedArea?.activeTab?.title == "Shell")
        #expect(presentedArea?.tabs.last?.isPinned == true)
        #expect(presentedArea?.tabs.last?.kind == .browser)
        #expect(presentedArea?.tabs.last?.content.pane == nil)
        #expect(presentedArea?.activeTab?.content.pane?.backend == .remoteMac(paneID: paneID))
    }

    @Test("remote project DTOs use normal Project models with a distinct origin")
    func projectHydration() {
        let identities = RemoteWorkspaceIdentityMap()
        let deviceID = UUID()
        let dto = ProjectDTO(
            id: UUID(),
            name: "Muxy",
            path: "/repo",
            sortOrder: 2,
            createdAt: .now,
            icon: "terminal",
            iconColor: "blue"
        )

        let project = RemoteWorkspacePresentationBuilder.projects(
            from: [dto],
            deviceID: deviceID,
            identities: identities
        ).first

        #expect(project?.name == dto.name)
        #expect(project?.path == dto.path)
        #expect(project?.isRemoteMac == true)
        #expect(project?.remoteMacDeviceID == deviceID)
        #expect(project?.id != dto.id)
    }
}
