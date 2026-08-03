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
}
