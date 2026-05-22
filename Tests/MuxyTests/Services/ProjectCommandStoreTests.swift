import Foundation
import Testing

@testable import Muxy

@Suite("ProjectCommandStore")
@MainActor
struct ProjectCommandStoreTests {
    @Test("removeRun by paneID clears linked command")
    func removeRunByPaneID() {
        let store = ProjectCommandStore(
            persistence: ProjectCommandPersistenceStub(),
            discovery: ProjectCommandDiscovery(providers: [])
        )
        let command = ProjectCommand(id: "manual:test", name: "Test", command: "npm test", source: .manual)
        let projectID = UUID()
        let paneID = UUID()

        store.run(command, projectID: projectID, tabID: UUID(), areaID: UUID(), paneID: paneID)
        store.removeRun(paneID: paneID)

        #expect(store.run(for: command.id, projectID: projectID) == nil)
    }

    @Test("runs are scoped by project and command")
    func runsAreScopedByProjectAndCommand() {
        let store = ProjectCommandStore(
            persistence: ProjectCommandPersistenceStub(),
            discovery: ProjectCommandDiscovery(providers: [])
        )
        let command = ProjectCommand(id: "npm:test", name: "test", command: "npm run test", source: .npm)
        let firstProjectID = UUID()
        let secondProjectID = UUID()
        let firstPaneID = UUID()
        let secondPaneID = UUID()

        store.run(command, projectID: firstProjectID, tabID: UUID(), areaID: UUID(), paneID: firstPaneID)
        store.run(command, projectID: secondProjectID, tabID: UUID(), areaID: UUID(), paneID: secondPaneID)

        #expect(store.run(for: command.id, projectID: firstProjectID)?.paneID == firstPaneID)
        #expect(store.run(for: command.id, projectID: secondProjectID)?.paneID == secondPaneID)
    }

    @Test("delete hides discovered command")
    func deleteHidesDiscoveredCommand() {
        let command = ProjectCommand(id: "npm:test", name: "test", command: "npm run test", source: .npm)
        let project = Project(name: "App", path: "/tmp/app")
        let persistence = ProjectCommandPersistenceStub()
        let store = ProjectCommandStore(
            persistence: persistence,
            discovery: ProjectCommandDiscovery(providers: [ProjectCommandProviderStub(commands: [command])])
        )

        store.delete(command, from: project.id)

        #expect(store.commands(for: project).isEmpty)
        #expect(persistence.savedHiddenDiscoveredCommandIDs[project.id] == [command.id])
    }

    @Test("loadDiscoveredCommands restores hidden project commands")
    func loadDiscoveredCommandsRestoresHiddenCommands() {
        let command = ProjectCommand(id: "npm:test", name: "test", command: "npm run test", source: .npm)
        let project = Project(name: "App", path: "/tmp/app")
        let persistence = ProjectCommandPersistenceStub()
        let store = ProjectCommandStore(
            persistence: persistence,
            discovery: ProjectCommandDiscovery(providers: [ProjectCommandProviderStub(commands: [command])])
        )

        store.delete(command, from: project.id)
        store.loadDiscoveredCommands(from: project)

        #expect(store.commands(for: project) == [command])
        #expect((persistence.savedHiddenDiscoveredCommandIDs[project.id] ?? []) == [])
    }

    @Test("loadDiscoveredCommands removes matching manual duplicate")
    func loadDiscoveredCommandsRemovesMatchingManualDuplicate() {
        let command = ProjectCommand(id: "npm:test", name: "test", command: "npm run test", source: .npm)
        let project = Project(name: "App", path: "/tmp/app")
        let persistence = ProjectCommandPersistenceStub()
        let store = ProjectCommandStore(
            persistence: persistence,
            discovery: ProjectCommandDiscovery(providers: [ProjectCommandProviderStub(commands: [command])])
        )

        store.addManualCommand(name: command.name, command: command.command, to: project.id)
        store.loadDiscoveredCommands(from: project)

        #expect(store.commands(for: project) == [command])
        #expect((persistence.savedManualCommands[project.id] ?? []) == [])
    }
}

private final class ProjectCommandPersistenceStub: ProjectCommandPersisting {
    var savedManualCommands: [UUID: [ProjectCommand]] = [:]
    var savedHiddenDiscoveredCommandIDs: [UUID: Set<String>] = [:]

    func loadManualCommands() throws -> [UUID: [ProjectCommand]] { savedManualCommands }
    func saveManualCommands(_ commands: [UUID: [ProjectCommand]]) throws {
        savedManualCommands = commands
    }

    func loadHiddenDiscoveredCommandIDs() throws -> [UUID: Set<String>] { savedHiddenDiscoveredCommandIDs }
    func saveHiddenDiscoveredCommandIDs(_ ids: [UUID: Set<String>]) throws {
        savedHiddenDiscoveredCommandIDs = ids
    }
}

private struct ProjectCommandProviderStub: ProjectCommandProviding {
    let commands: [ProjectCommand]

    func commands(projectPath: String) -> [ProjectCommand] {
        commands
    }
}
