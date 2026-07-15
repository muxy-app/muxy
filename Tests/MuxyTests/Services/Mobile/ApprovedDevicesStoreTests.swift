import Foundation
import MuxyShared
import Testing

@testable import Muxy

@Suite("ApprovedDevicesStore")
@MainActor
struct ApprovedDevicesStoreTests {
    private final class RevokedRecorder {
        private(set) var ids: [UUID] = []
        func record(_ id: UUID) { ids.append(id) }
    }

    private func makeStore(deviceCount: Int) -> (ApprovedDevicesStore, [ApprovedDevice], RevokedRecorder) {
        let store = ApprovedDevicesStore(persistence: ApprovedDevicesPersistenceStub())
        for index in 0 ..< deviceCount {
            store.approve(deviceID: UUID(), name: "Device \(index)", token: "token-\(index)")
        }
        let recorder = RevokedRecorder()
        store.onRevoke = { recorder.record($0) }
        return (store, store.devices, recorder)
    }

    @Test("batch revoke removes only the selected devices")
    func batchRevokeRemovesSelected() {
        let (store, seeded, _) = makeStore(deviceCount: 3)
        store.revoke(deviceIDs: [seeded[0].id, seeded[2].id])

        #expect(store.devices.map(\.id) == [seeded[1].id])
    }

    @Test("batch revoke fires onRevoke once per removed device")
    func batchRevokeFiresCallbackPerDevice() {
        let (store, seeded, recorder) = makeStore(deviceCount: 3)
        let toRemove: Set<UUID> = [seeded[0].id, seeded[1].id]
        store.revoke(deviceIDs: toRemove)

        #expect(Set(recorder.ids) == toRemove)
        #expect(recorder.ids.count == 2)
    }

    @Test("batch revoke with empty set is a no-op")
    func batchRevokeEmptySetNoOp() {
        let (store, seeded, recorder) = makeStore(deviceCount: 2)
        store.revoke(deviceIDs: [])

        #expect(store.devices.map(\.id) == seeded.map(\.id))
        #expect(recorder.ids.isEmpty)
    }

    @Test("batch revoke ignores unknown ids")
    func batchRevokeUnknownIDsNoOp() {
        let (store, seeded, recorder) = makeStore(deviceCount: 2)
        store.revoke(deviceIDs: [UUID()])

        #expect(store.devices.map(\.id) == seeded.map(\.id))
        #expect(recorder.ids.isEmpty)
    }

    @Test("single revoke removes exactly one device and fires once")
    func singleRevokeDelegates() {
        let (store, seeded, recorder) = makeStore(deviceCount: 3)
        store.revoke(deviceID: seeded[1].id)

        #expect(store.devices.map(\.id) == [seeded[0].id, seeded[2].id])
        #expect(recorder.ids == [seeded[1].id])
    }

    @Test("approved devices preserve desktop client kind")
    func desktopClientKind() {
        let store = ApprovedDevicesStore(persistence: ApprovedDevicesPersistenceStub())
        let deviceID = UUID()
        store.approve(deviceID: deviceID, name: "Mac", token: "token", clientKind: .desktop)

        #expect(store.devices.first?.clientKind == .desktop)
        #expect(store.validate(deviceID: deviceID, token: "token", clientKind: .desktop) != nil)
        #expect(store.validate(deviceID: deviceID, token: "token", clientKind: .mobile) == nil)
    }

    @Test("legacy approved devices default to mobile")
    func legacyClientKind() throws {
        let data = Data(#"{"id":"00000000-0000-0000-0000-000000000001","name":"Phone","tokenHash":"hash","approvedAt":0}"#.utf8)
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        let device = try decoder.decode(ApprovedDevice.self, from: data)

        #expect(device.clientKind == .mobile)
    }

    @Test("desktop and mobile access toggles are enforced independently")
    func clientAccessPolicy() {
        #expect(MobileServerService.allows(.desktop, mobileConnections: false, desktopConnections: true))
        #expect(!MobileServerService.allows(.desktop, mobileConnections: true, desktopConnections: false))
        #expect(MobileServerService.allows(.mobile, mobileConnections: true, desktopConnections: false))
        #expect(!MobileServerService.allows(.mobile, mobileConnections: false, desktopConnections: true))
    }
}

private final class ApprovedDevicesPersistenceStub: ApprovedDevicesPersisting {
    private var devices: [ApprovedDevice] = []

    func loadDevices() throws -> [ApprovedDevice] {
        devices
    }

    func saveDevices(_ devices: [ApprovedDevice]) throws {
        self.devices = devices
    }
}
