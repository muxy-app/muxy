import Foundation
import Testing

@testable import Muxy

@Suite("RemoteProjectPickerFileSystem parsing")
struct RemoteProjectPickerParsingTests {
    @Test("parses NUL-delimited directory and file entries")
    func parsesEntries() {
        let output = "d src\u{0}f README.md\u{0}d node_modules\u{0}"
        let entries = RemoteProjectPickerFileSystem.parseEntries(output)
        #expect(entries == [.directory("src"), .file("README.md"), .directory("node_modules")])
    }

    @Test("ignores malformed records")
    func ignoresMalformed() {
        let output = "d ok\u{0}x\u{0}\u{0}d good\u{0}"
        let entries = RemoteProjectPickerFileSystem.parseEntries(output)
        #expect(entries == [.directory("ok"), .directory("good")])
    }

    @Test("keeps names with spaces")
    func keepsSpaces() {
        let output = "d My Project\u{0}"
        let entries = RemoteProjectPickerFileSystem.parseEntries(output)
        #expect(entries == [.directory("My Project")])
    }
}

@Suite("Remote path standardization")
struct RemotePathStandardizationTests {
    @Test("collapses dot segments without touching the local filesystem")
    func collapsesDots() {
        #expect(ProjectPickerPathService.standardizedRemotePath("~/code/./api") == "~/code/api")
        #expect(ProjectPickerPathService.standardizedRemotePath("/srv/app/../app") == "/srv/app")
        #expect(ProjectPickerPathService.standardizedRemotePath("~/code/") == "~/code")
    }

    @Test("preserves the leading tilde for remote expansion")
    func preservesTilde() {
        #expect(ProjectPickerPathService.standardizedRemotePath("~") == "~")
        #expect(ProjectPickerPathService.standardizedRemotePath("~/projects") == "~/projects")
    }
}

@Suite("ProjectGroupStore workspace context")
@MainActor
struct ProjectGroupStoreContextTests {
    private let deviceStore = RemoteDeviceStore(persistence: InMemoryRemoteDevicePersistence())

    private func makeStore() -> ProjectGroupStore {
        ProjectGroupStore(
            persistence: InMemoryProjectGroupPersistence(),
            remoteDeviceStore: deviceStore,
            workspaceContextSink: InMemoryWorkspaceContextSink()
        )
    }

    private func makeSSHWorkspace(in store: ProjectGroupStore, remoteRoot: String) -> ProjectGroup {
        let device = deviceStore.add(name: "prod", ssh: SSHWorkspaceData(host: "prod", remoteRoot: remoteRoot))
        return store.addRemoteWorkspace(name: "prod", deviceID: device.id)
    }

    @Test("ssh workspace selection derives an ssh context")
    func sshContext() {
        let store = makeStore()
        let group = makeSSHWorkspace(in: store, remoteRoot: "~/code")
        store.selectGroup(id: group.id)
        #expect(store.activeWorkspaceContext == .ssh(SSHDestination(host: "prod", remoteRoot: "~/code")))
        #expect(store.isRemoteWorkspaceActive)
    }

    @Test("clearing selection returns to local context")
    func localContext() {
        let store = makeStore()
        let group = makeSSHWorkspace(in: store, remoteRoot: "~")
        store.selectGroup(id: group.id)
        store.clearGroupSelection()
        #expect(store.activeWorkspaceContext == .local)
        #expect(!store.isRemoteWorkspaceActive)
    }

    @Test("local project keeps a local context while a remote workspace is active")
    func localProjectStaysLocalUnderRemoteWorkspace() {
        let store = makeStore()
        let group = makeSSHWorkspace(in: store, remoteRoot: "~")
        store.selectGroup(id: group.id)
        let localProject = Project(name: "local", path: "/tmp/local")
        #expect(store.workspaceContext(for: localProject) == .local)
    }

    @Test("remote project resolves its workspace context regardless of active selection")
    func remoteProjectResolvesContextWithoutActiveSelection() throws {
        let store = makeStore()
        let group = makeSSHWorkspace(in: store, remoteRoot: "~/code")
        let remote = try #require(store.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id))
        let project = remote.asProject(workspaceID: group.id, sortOrder: 0)
        store.clearGroupSelection()
        #expect(store.workspaceContext(for: project) == .ssh(SSHDestination(host: "prod", remoteRoot: "~/code")))
    }

    @Test("ssh workspace hides local projects and surfaces remote ones")
    func displayProjects() {
        let store = makeStore()
        let group = makeSSHWorkspace(in: store, remoteRoot: "~")
        store.addRemoteProject(name: "api", path: "~/code/api", toGroup: group.id)
        store.selectGroup(id: group.id)
        let locals = [Project(name: "local", path: "/tmp/local")]
        let displayed = store.displayProjects(localProjects: locals)
        #expect(displayed.count == 1)
        #expect(displayed.first?.path == "~/code/api")
        #expect(displayed.first?.isRemote == true)
    }
}

private final class InMemoryProjectGroupPersistence: ProjectGroupPersisting {
    private var groups: [ProjectGroup] = []
    private var activeID: UUID?

    func loadProjectGroups() throws -> [ProjectGroup] { groups }
    func saveProjectGroups(_ groups: [ProjectGroup]) throws { self.groups = groups }
    func loadActiveGroupID() -> UUID? { activeID }
    func saveActiveGroupID(_ id: UUID?) { activeID = id }
}
