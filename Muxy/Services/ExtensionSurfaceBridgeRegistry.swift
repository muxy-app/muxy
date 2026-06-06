import Foundation

@MainActor
final class ExtensionSurfaceBridgeRegistry {
    static let shared = ExtensionSurfaceBridgeRegistry()

    private var bridges: [LifecycleSurfaceKey: any BeforeCloseAsking] = [:]

    func register(_ bridge: any BeforeCloseAsking, for key: LifecycleSurfaceKey) {
        bridges[key] = bridge
    }

    func unregister(_ key: LifecycleSurfaceKey) {
        guard let bridge = bridges.removeValue(forKey: key) else { return }
        bridge.failPendingLifecycle()
    }

    func requestBeforeClose(_ key: LifecycleSurfaceKey) async -> LifecycleVerdict {
        guard let bridge = bridges[key] else { return .allow }
        return await bridge.requestBeforeClose(reason: key.kind, instanceID: key.instanceID)
    }
}
