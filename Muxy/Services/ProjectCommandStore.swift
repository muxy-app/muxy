import Foundation
import Observation

@MainActor
@Observable
final class ProjectCommandStore {
    private let persistence: any ProjectCommandPersisting
    private let discovery: ProjectCommandDiscovery
    private var manualCommands: [UUID: [ProjectCommand]]
    private var hiddenDiscoveredCommandIDs: [UUID: Set<String>]
    private(set) var runs: [String: ProjectCommandRun] = [:]

    init(
        persistence: any ProjectCommandPersisting = FileProjectCommandPersistence(),
        discovery: ProjectCommandDiscovery = ProjectCommandDiscovery()
    ) {
        self.persistence = persistence
        self.discovery = discovery
        manualCommands = (try? persistence.loadManualCommands()) ?? [:]
        hiddenDiscoveredCommandIDs = (try? persistence.loadHiddenDiscoveredCommandIDs()) ?? [:]
    }

    func commands(for project: Project) -> [ProjectCommand] {
        let hiddenIDs = hiddenDiscoveredCommandIDs[project.id] ?? []
        let discoveredCommands = discovery.commands(projectPath: project.path)
            .filter { !hiddenIDs.contains($0.id) }
        return discoveredCommands + (manualCommands[project.id] ?? [])
    }

    func addManualCommand(name: String, command: String, to projectID: UUID) {
        let trimmedName = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedCommand = command.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedCommand.isEmpty else { return }
        let command = ProjectCommand(
            id: "manual:\(UUID().uuidString)",
            name: trimmedName.isEmpty ? trimmedCommand : trimmedName,
            command: trimmedCommand,
            source: .manual
        )
        manualCommands[projectID, default: []].append(command)
        saveManualCommands()
    }

    func delete(_ command: ProjectCommand, from projectID: UUID) {
        switch command.source {
        case .manual:
            manualCommands[projectID, default: []].removeAll { $0.id == command.id }
            saveManualCommands()
        case .npm,
             .composer:
            hiddenDiscoveredCommandIDs[projectID, default: []].insert(command.id)
            saveHiddenDiscoveredCommandIDs()
        }
        runs.removeValue(forKey: command.id)
    }

    func loadDiscoveredCommands(from project: Project) {
        let discoveredCommands = discovery.commands(projectPath: project.path)
        let discoveredSignatures = Set(discoveredCommands.map(ProjectCommandSignature.init))
        manualCommands[project.id, default: []].removeAll { discoveredSignatures.contains(ProjectCommandSignature($0)) }
        hiddenDiscoveredCommandIDs[project.id] = []
        saveManualCommands()
        saveHiddenDiscoveredCommandIDs()
    }

    func run(_ command: ProjectCommand, projectID: UUID, tabID: UUID, areaID: UUID, paneID: UUID) {
        runs[command.id] = ProjectCommandRun(
            commandID: command.id,
            projectID: projectID,
            tabID: tabID,
            areaID: areaID,
            paneID: paneID,
            state: .running
        )
    }

    func replaceRun(_ run: ProjectCommandRun) {
        runs[run.commandID] = run
    }

    func markStopped(_ commandID: String) {
        guard var run = runs[commandID] else { return }
        run.state = .stopped
        runs[commandID] = run
    }

    func removeRun(_ commandID: String) {
        runs.removeValue(forKey: commandID)
    }

    func removeRun(paneID: UUID) {
        guard let commandID = runs.first(where: { $0.value.paneID == paneID })?.key else { return }
        runs.removeValue(forKey: commandID)
    }

    private func saveManualCommands() {
        try? persistence.saveManualCommands(manualCommands)
    }

    private func saveHiddenDiscoveredCommandIDs() {
        try? persistence.saveHiddenDiscoveredCommandIDs(hiddenDiscoveredCommandIDs)
    }
}

private struct ProjectCommandSignature: Hashable {
    let name: String
    let command: String

    init(_ command: ProjectCommand) {
        name = command.name
        self.command = command.command
    }
}
