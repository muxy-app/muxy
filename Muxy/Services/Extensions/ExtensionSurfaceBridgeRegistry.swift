import Foundation

@MainActor
final class ExtensionSurfaceBridgeRegistry {
    static let shared = ExtensionSurfaceBridgeRegistry()

    private var bridges: [LifecycleSurfaceKey: any BeforeCloseAsking] = [:]

    func register(_ bridge: any BeforeCloseAsking, for key: LifecycleSurfaceKey) {
        let replacedBridge = bridges.updateValue(bridge, forKey: key)
        guard let replacedBridge,
              ObjectIdentifier(replacedBridge) != ObjectIdentifier(bridge)
        else { return }
        replacedBridge.failPendingLifecycle()
    }

    func unregister(
        _ key: LifecycleSurfaceKey,
        ifMatches expectedBridge: (any BeforeCloseAsking)? = nil
    ) {
        if let expectedBridge,
           let registeredBridge = bridges[key],
           ObjectIdentifier(registeredBridge) != ObjectIdentifier(expectedBridge)
        {
            return
        }
        guard let bridge = bridges.removeValue(forKey: key) else { return }
        bridge.failPendingLifecycle()
    }

    func requestBeforeClose(_ key: LifecycleSurfaceKey) async -> LifecycleVerdict {
        guard let bridge = bridges[key] else { return .allow }
        return await bridge.requestBeforeClose(reason: key.kind, instanceID: key.instanceID)
    }
}
