import Foundation
import Testing

@testable import Muxy

@Suite("DetectedAgentStore")
@MainActor
struct DetectedAgentStoreTests {
    @Test("sets and reads a detected agent")
    func setsAndReads() {
        let store = DetectedAgentStore.shared
        let paneID = UUID()
        store.setAgent("claude", for: paneID)
        #expect(store.agent(for: paneID) == "claude")
        store.resetPane(paneID)
    }

    @Test("overwrites an existing agent")
    func overwrites() {
        let store = DetectedAgentStore.shared
        let paneID = UUID()
        store.setAgent("claude", for: paneID)
        store.setAgent("codex", for: paneID)
        #expect(store.agent(for: paneID) == "codex")
        store.resetPane(paneID)
    }

    @Test("clears an agent when set to nil")
    func clearsOnNil() {
        let store = DetectedAgentStore.shared
        let paneID = UUID()
        store.setAgent("claude", for: paneID)
        store.setAgent(nil, for: paneID)
        #expect(store.agent(for: paneID) == nil)
    }

    @Test("resetPane removes the entry")
    func resetRemoves() {
        let store = DetectedAgentStore.shared
        let paneID = UUID()
        store.setAgent("cursor", for: paneID)
        store.resetPane(paneID)
        #expect(store.agent(for: paneID) == nil)
    }

    @Test("resolves an icon name for a known provider")
    func resolvesIconName() {
        let store = DetectedAgentStore.shared
        let paneID = UUID()
        store.setAgent("claude", for: paneID)
        #expect(store.iconName(forPane: paneID) == "claude")
        store.resetPane(paneID)
    }

    @Test("returns nil icon name for a pane without a detected agent")
    func nilIconNameWhenAbsent() {
        let store = DetectedAgentStore.shared
        #expect(store.iconName(forPane: UUID()) == nil)
        #expect(store.iconName(forPane: nil) == nil)
    }
}
