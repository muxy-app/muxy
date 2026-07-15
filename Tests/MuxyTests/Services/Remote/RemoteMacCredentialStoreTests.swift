import Foundation
import Testing

@testable import Muxy

@Suite("Remote Mac credential store")
struct RemoteMacCredentialStoreTests {
    @Test("credentials are stable and scoped per remote device")
    func scopedCredentials() throws {
        let store = InMemoryRemoteMacCredentialStore()
        let firstDevice = UUID()
        let secondDevice = UUID()

        let first = try store.loadOrCreate(for: firstDevice, endpointScope: "studio.local:4865")
        let firstReloaded = try store.loadOrCreate(for: firstDevice, endpointScope: "studio.local:4865")
        let second = try store.loadOrCreate(for: secondDevice, endpointScope: "studio.local:4865")

        #expect(first == firstReloaded)
        #expect(first != second)
        #expect(!first.token.isEmpty)
    }

    @Test("deleting a credential creates a new identity")
    func deletion() throws {
        let store = InMemoryRemoteMacCredentialStore()
        let deviceID = UUID()
        let original = try store.loadOrCreate(for: deviceID, endpointScope: "studio.local:4865")

        try store.delete(for: deviceID)
        let replacement = try store.loadOrCreate(for: deviceID, endpointScope: "studio.local:4865")

        #expect(original != replacement)
    }

    @Test("credentials remain independently scoped when endpoints change")
    func endpointBinding() throws {
        let store = InMemoryRemoteMacCredentialStore()
        let deviceID = UUID()
        let original = try store.loadOrCreate(for: deviceID, endpointScope: "studio.local:4865")

        let replacement = try store.loadOrCreate(for: deviceID, endpointScope: "laptop.local:4865")
        let originalReloaded = try store.loadOrCreate(for: deviceID, endpointScope: "studio.local:4865")

        #expect(original != replacement)
        #expect(original == originalReloaded)
        #expect(replacement.endpointScope == "laptop.local:4865")
    }

    @Test("deleting one endpoint preserves credentials for other endpoints")
    func endpointDeletion() throws {
        let store = InMemoryRemoteMacCredentialStore()
        let deviceID = UUID()
        let first = try store.loadOrCreate(for: deviceID, endpointScope: "studio.local:4865")
        let second = try store.loadOrCreate(for: deviceID, endpointScope: "laptop.local:4865")

        try store.delete(for: deviceID, endpointScope: "laptop.local:4865")

        let firstReloaded = try store.loadOrCreate(for: deviceID, endpointScope: "studio.local:4865")
        let secondReplacement = try store.loadOrCreate(for: deviceID, endpointScope: "laptop.local:4865")
        #expect(first == firstReloaded)
        #expect(second != secondReplacement)
    }
}
