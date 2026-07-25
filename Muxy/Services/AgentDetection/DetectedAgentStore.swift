import Foundation
import Observation

@MainActor
@Observable
final class DetectedAgentStore {
    static let shared = DetectedAgentStore()

    private(set) var agents: [UUID: String] = [:]
    private(set) var sessionIDs: [UUID: String] = [:]

    private init() {}

    static func executablesSnapshot(from registry: AIProviderRegistry) -> [AIAgentExecutable] {
        registry.detectableProviders.map {
            AIAgentExecutable(providerID: $0.id, executableNames: $0.executableNames)
        }
    }

    func setAgent(_ providerID: String?, for paneID: UUID) {
        guard agents[paneID] != providerID else { return }
        if let providerID {
            agents[paneID] = providerID
            return
        }
        agents.removeValue(forKey: paneID)
    }

    func agent(for paneID: UUID) -> String? {
        agents[paneID]
    }

    func setSession(_ sessionID: String?, for paneID: UUID) {
        guard let sessionID, !sessionID.isEmpty else { sessionIDs.removeValue(forKey: paneID); return }
        sessionIDs[paneID] = sessionID
    }

    func sessionID(for paneID: UUID) -> String? { sessionIDs[paneID] }

    func iconName(forPane paneID: UUID?) -> String? {
        guard let paneID, let providerID = agents[paneID] else { return nil }
        return AIProviderRegistry.shared.iconName(forProviderID: providerID)
    }

    func resetPane(_ paneID: UUID) {
        agents.removeValue(forKey: paneID)
        sessionIDs.removeValue(forKey: paneID)
    }
}
