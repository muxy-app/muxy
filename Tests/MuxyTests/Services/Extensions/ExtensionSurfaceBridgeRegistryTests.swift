import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionSurfaceBridgeRegistry")
@MainActor
struct ExtensionSurfaceBridgeRegistryTests {
    @Test("returns allow when no surface is registered for the key")
    func failsOpenWhenUnregistered() async {
        let registry = ExtensionSurfaceBridgeRegistry()
        let key = LifecycleSurfaceKey(kind: .tab, instanceID: "missing")
        let verdict = await registry.requestBeforeClose(key)
        #expect(verdict == .allow)
    }

    @Test("forwards the registered bridge's verdict")
    func forwardsVerdict() async {
        let registry = ExtensionSurfaceBridgeRegistry()
        let key = LifecycleSurfaceKey(kind: .panel, instanceID: "p1")
        let bridge = FakeBeforeCloseAsking(verdict: .prevent)
        registry.register(bridge, for: key)

        let verdict = await registry.requestBeforeClose(key)
        #expect(verdict == .prevent)
        #expect(bridge.askedReason == .panel)
        #expect(bridge.askedInstanceID == "p1")
    }

    @Test("unregister sweeps the bridge's pending continuations")
    func unregisterFailsPending() {
        let registry = ExtensionSurfaceBridgeRegistry()
        let key = LifecycleSurfaceKey(kind: .popover, instanceID: "x")
        let bridge = FakeBeforeCloseAsking(verdict: .allow)
        registry.register(bridge, for: key)

        registry.unregister(key)
        #expect(bridge.failPendingCallCount == 1)
    }

    @Test("unregister of an unknown key does nothing")
    func unregisterUnknownIsNoOp() {
        let registry = ExtensionSurfaceBridgeRegistry()
        registry.unregister(LifecycleSurfaceKey(kind: .tab, instanceID: "nope"))
    }

    @Test("stale unregister preserves the replacement bridge")
    func staleUnregisterPreservesReplacement() async {
        let registry = ExtensionSurfaceBridgeRegistry()
        let key = LifecycleSurfaceKey(kind: .tab, instanceID: "replacement")
        let staleBridge = FakeBeforeCloseAsking(verdict: .allow)
        let replacementBridge = FakeBeforeCloseAsking(verdict: .prevent)
        registry.register(staleBridge, for: key)
        registry.register(replacementBridge, for: key)

        registry.unregister(key, ifMatches: staleBridge)
        let verdict = await registry.requestBeforeClose(key)

        #expect(verdict == .prevent)
        #expect(staleBridge.failPendingCallCount == 1)
        #expect(replacementBridge.failPendingCallCount == 0)
    }
}

@MainActor
private final class FakeBeforeCloseAsking: BeforeCloseAsking {
    let verdict: LifecycleVerdict
    private(set) var askedReason: LifecycleSurfaceKind?
    private(set) var askedInstanceID: String?
    private(set) var failPendingCallCount = 0

    init(verdict: LifecycleVerdict) {
        self.verdict = verdict
    }

    func requestBeforeClose(reason: LifecycleSurfaceKind, instanceID: String) async -> LifecycleVerdict {
        askedReason = reason
        askedInstanceID = instanceID
        return verdict
    }

    func failPendingLifecycle() {
        failPendingCallCount += 1
    }
}
