import Foundation

@MainActor
enum AgentPaneIdentity {
    static func providerID(
        forPane paneID: UUID?,
        detectedAgentStore: DetectedAgentStore = .shared,
        agentStatusStore: AgentStatusStore = .shared
    ) -> String? {
        guard let paneID else { return nil }
        return detectedAgentStore.agent(for: paneID) ?? agentStatusStore.activeProviderID(forPane: paneID)
    }

    static func providerID(
        forPanes paneIDs: [UUID],
        detectedAgentStore: DetectedAgentStore = .shared,
        agentStatusStore: AgentStatusStore = .shared
    ) -> String? {
        for paneID in paneIDs {
            if let providerID = detectedAgentStore.agent(for: paneID) {
                return providerID
            }
        }
        for paneID in paneIDs {
            if let providerID = agentStatusStore.activeProviderID(forPane: paneID) {
                return providerID
            }
        }
        return nil
    }

    static func iconName(
        forPane paneID: UUID?,
        detectedAgentStore: DetectedAgentStore = .shared,
        agentStatusStore: AgentStatusStore = .shared,
        registry: AIProviderRegistry = .shared
    ) -> String? {
        guard let providerID = providerID(
            forPane: paneID,
            detectedAgentStore: detectedAgentStore,
            agentStatusStore: agentStatusStore
        )
        else { return nil }
        return registry.iconName(forProviderID: providerID)
    }

    static func iconName(
        forPanes paneIDs: [UUID],
        detectedAgentStore: DetectedAgentStore = .shared,
        agentStatusStore: AgentStatusStore = .shared,
        registry: AIProviderRegistry = .shared
    ) -> String? {
        guard let providerID = providerID(
            forPanes: paneIDs,
            detectedAgentStore: detectedAgentStore,
            agentStatusStore: agentStatusStore
        )
        else { return nil }
        return registry.iconName(forProviderID: providerID)
    }
}
