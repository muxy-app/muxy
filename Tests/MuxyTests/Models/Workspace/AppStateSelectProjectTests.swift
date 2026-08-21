import Foundation
import Testing

@testable import Muxy

@Suite("AppState.selectProject")
@MainActor
struct AppStateSelectProjectTests {
    @Test("selecting a new project notifies onWorkspaceSelected")
    func notifiesOnNewSelection() {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState()
        var selected: [WorktreeKey] = []
        appState.onWorkspaceSelected = { selected.append($0) }

        appState.selectProject(project, worktree: worktree)

        #expect(selected == [WorktreeKey(projectID: project.id, worktreeID: worktree.id)])
    }

    @Test("reselecting the active workspace does not notify again")
    func skipsNotificationWhenAlreadyActive() {
        let project = Project(name: "api", path: "/tmp/api")
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState()
        var selected: [WorktreeKey] = []
        appState.onWorkspaceSelected = { selected.append($0) }

        appState.selectProject(project, worktree: worktree)
        appState.selectProject(project, worktree: worktree)

        #expect(selected == [WorktreeKey(projectID: project.id, worktreeID: worktree.id)])
    }

    @Test("selecting another worktree notifies onWorkspaceSelected")
    func notifiesOnWorktreeSelection() {
        let project = Project(name: "api", path: "/tmp/api")
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "feature", path: "/tmp/api-feature", isPrimary: false)
        let appState = makeAppState()
        var selected: [WorktreeKey] = []
        appState.onWorkspaceSelected = { selected.append($0) }

        appState.selectProject(project, worktree: primary)
        appState.selectWorktree(projectID: project.id, worktree: feature)

        #expect(selected == [
            WorktreeKey(projectID: project.id, worktreeID: primary.id),
            WorktreeKey(projectID: project.id, worktreeID: feature.id),
        ])
    }

    @Test("cycling projects notifies onWorkspaceSelected")
    func notifiesWhenCyclingProjects() {
        let projectA = Project(name: "api", path: "/tmp/api")
        let projectB = Project(name: "web", path: "/tmp/web")
        let worktreeA = Worktree(name: projectA.name, path: projectA.path, isPrimary: true)
        let worktreeB = Worktree(name: projectB.name, path: projectB.path, isPrimary: true)
        let appState = makeAppState()
        var selected: [WorktreeKey] = []
        appState.onWorkspaceSelected = { selected.append($0) }

        appState.selectProject(projectA, worktree: worktreeA)
        appState.selectNextProject(
            projects: [projectA, projectB],
            worktrees: [projectA.id: [worktreeA], projectB.id: [worktreeB]]
        )

        #expect(selected == [
            WorktreeKey(projectID: projectA.id, worktreeID: worktreeA.id),
            WorktreeKey(projectID: projectB.id, worktreeID: worktreeB.id),
        ])
    }

    @Test("selecting projects swaps extension panels via the registry")
    func selectProjectSwapsExtensionPanels() {
        let projectA = Project(name: "api", path: "/tmp/api-\(UUID().uuidString)")
        let projectB = Project(name: "web", path: "/tmp/web-\(UUID().uuidString)")
        let worktreeA = Worktree(name: projectA.name, path: projectA.path, isPrimary: true)
        let worktreeB = Worktree(name: projectB.name, path: projectB.path, isPrimary: true)
        let appState = makeAppState()
        let registry = ExtensionPanelRegistry.shared
        let extensionA = "appstate-files-\(UUID().uuidString)"
        let extensionB = "appstate-git-\(UUID().uuidString)"
        defer {
            registry.closeAll(extensionID: extensionA)
            registry.closeAll(extensionID: extensionB)
            registry.activateProject(nil, from: registry.activeProjectID)
        }

        appState.selectProject(projectA, worktree: worktreeA)
        registry.open(
            extensionID: extensionA,
            panel: ExtensionPanel(id: "files", entry: "index.html", mode: .pinned),
            data: nil
        )

        appState.selectProject(projectB, worktree: worktreeB)
        #expect(!PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
            extensionID: extensionA,
            panelID: "files"
        )))
        registry.open(
            extensionID: extensionB,
            panel: ExtensionPanel(id: "changes", entry: "index.html", mode: .floating),
            data: nil
        )

        appState.selectProject(projectA, worktree: worktreeA)
        #expect(PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
            extensionID: extensionA,
            panelID: "files"
        )))
        #expect(!PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
            extensionID: extensionB,
            panelID: "changes"
        )))
    }

    @Test("selectNextProject swaps extension panels")
    func selectNextProjectSwapsExtensionPanels() {
        let projectA = Project(name: "api", path: "/tmp/api-\(UUID().uuidString)")
        let projectB = Project(name: "web", path: "/tmp/web-\(UUID().uuidString)")
        let worktreeA = Worktree(name: projectA.name, path: projectA.path, isPrimary: true)
        let worktreeB = Worktree(name: projectB.name, path: projectB.path, isPrimary: true)
        let appState = makeAppState()
        let registry = ExtensionPanelRegistry.shared
        let extensionID = "appstate-cycle-\(UUID().uuidString)"
        defer {
            registry.closeAll(extensionID: extensionID)
            registry.activateProject(nil, from: registry.activeProjectID)
        }

        appState.selectProject(projectA, worktree: worktreeA)
        registry.open(
            extensionID: extensionID,
            panel: ExtensionPanel(id: "files", entry: "index.html", mode: .pinned),
            data: nil
        )

        appState.selectNextProject(
            projects: [projectA, projectB],
            worktrees: [
                projectA.id: [worktreeA],
                projectB.id: [worktreeB],
            ]
        )
        #expect(!PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
            extensionID: extensionID,
            panelID: "files"
        )))

        appState.selectNextProject(
            projects: [projectA, projectB],
            worktrees: [
                projectA.id: [worktreeA],
                projectB.id: [worktreeB],
            ]
        )
        #expect(PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
            extensionID: extensionID,
            panelID: "files"
        )))
    }

    @Test("removing a project purges its panel snapshots")
    func removeProjectPurgesPanelSnapshots() {
        let projectA = Project(name: "api", path: "/tmp/api-\(UUID().uuidString)")
        let projectB = Project(name: "web", path: "/tmp/web-\(UUID().uuidString)")
        let worktreeA = Worktree(name: projectA.name, path: projectA.path, isPrimary: true)
        let worktreeB = Worktree(name: projectB.name, path: projectB.path, isPrimary: true)
        let appState = makeAppState()
        let registry = ExtensionPanelRegistry.shared
        let extensionID = "appstate-remove-\(UUID().uuidString)"
        defer {
            registry.closeAll(extensionID: extensionID)
            registry.activateProject(nil, from: registry.activeProjectID)
        }

        appState.selectProject(projectA, worktree: worktreeA)
        registry.open(
            extensionID: extensionID,
            panel: ExtensionPanel(id: "files", entry: "index.html", mode: .pinned),
            data: nil
        )
        appState.selectProject(projectB, worktree: worktreeB)
        appState.removeProject(projectA.id)
        appState.selectProject(projectA, worktree: worktreeA)

        #expect(!PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
            extensionID: extensionID,
            panelID: "files"
        )))
    }

    private func makeAppState() -> AppState {
        AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_ workspaces: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for paneID: UUID) {}
    func needsConfirmQuit(for paneID: UUID) -> Bool { false }
}
