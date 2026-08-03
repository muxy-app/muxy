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
        paneIDs.compactMap { detectedAgentStore.agent(for: $0) }.first
            ?? paneIDs.compactMap { agentStatusStore.activeProviderID(forPane: $0) }.first
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
