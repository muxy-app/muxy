import Foundation
import Testing

@testable import Muxy

@Suite("RemoteDeviceStore")
@MainActor
struct RemoteDeviceStoreTests {
    private func makeStore(_ initial: [RemoteDevice] = []) -> (RemoteDeviceStore, InMemoryRemoteDevicePersistence) {
        let persistence = InMemoryRemoteDevicePersistence(initial: initial)
        return (RemoteDeviceStore(persistence: persistence), persistence)
    }

    private func tempURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("RemoteDeviceStoreTests-\(UUID().uuidString).json")
    }

    @Test("add appends a device and persists")
    func add() throws {
        let (store, persistence) = makeStore()

        let device = store.add(name: "Prod", ssh: SSHWorkspaceData(host: "example.com"))

        #expect(store.devices.map(\.id) == [device.id])
        #expect(try persistence.loadDevices().count == 1)
    }

    @Test("device(id:) resolves a stored device and nil for unknown")
    func deviceLookup() {
        let device = RemoteDevice(name: "Prod", ssh: SSHWorkspaceData(host: "example.com"))
        let (store, _) = makeStore([device])

        #expect(store.device(id: device.id)?.id == device.id)
        #expect(store.device(id: UUID()) == nil)
        #expect(store.device(id: nil) == nil)
    }

    @Test("sshDevices returns only ssh-kind devices")
    func sshDevicesFilter() {
        let ssh = RemoteDevice(name: "Prod", kind: .ssh, ssh: SSHWorkspaceData(host: "example.com"))
        let (store, _) = makeStore([ssh])

        #expect(store.sshDevices().map(\.id) == [ssh.id])
    }

    @Test("update mutates the device and persists")
    func update() throws {
        let device = RemoteDevice(name: "Prod", ssh: SSHWorkspaceData(host: "old.com"))
        let (store, persistence) = makeStore([device])

        store.update(id: device.id) {
            $0.name = "Renamed"
            $0.ssh = SSHWorkspaceData(host: "new.com", remoteRoot: "~/work")
        }

        #expect(store.device(id: device.id)?.name == "Renamed")
        #expect(store.device(id: device.id)?.ssh.host == "new.com")
        #expect(try persistence.loadDevices().first?.ssh.host == "new.com")
    }

    @Test("rename ignores blank names")
    func renameBlank() {
        let device = RemoteDevice(name: "Prod", ssh: SSHWorkspaceData(host: "example.com"))
        let (store, _) = makeStore([device])

        store.rename(id: device.id, to: "   ")

        #expect(store.device(id: device.id)?.name == "Prod")
    }

    @Test("remove deletes the device and persists")
    func remove() throws {
        let device = RemoteDevice(name: "Prod", ssh: SSHWorkspaceData(host: "example.com"))
        let (store, persistence) = makeStore([device])

        store.remove(id: device.id)

        #expect(store.devices.isEmpty)
        #expect(try persistence.loadDevices().isEmpty)
    }

    @Test("devices round-trip through persistence")
    func persistenceRoundTrip() throws {
        let (store, persistence) = makeStore()
        store.add(
            name: "A",
            ssh: SSHWorkspaceData(
                host: "a.com",
                remoteRoot: "~/a",
                port: 2222,
                user: "deploy",
                environment: ["TERM": "screen-256color", "LANG": "C.UTF-8"]
            )
        )

        let reloaded = RemoteDeviceStore(persistence: persistence)

        #expect(reloaded.devices.count == 1)
        #expect(reloaded.devices.first?.ssh.port == 2222)
        #expect(reloaded.devices.first?.ssh.user == "deploy")
        #expect(reloaded.devices.first?.ssh.environment["TERM"] == "screen-256color")
    }

    @Test("file persistence round-trips environment")
    func filePersistenceRoundTrip() throws {
        let url = tempURL()
        let persistence = FileRemoteDevicePersistence(fileURL: url)
        let device = RemoteDevice(
            name: "Prod",
            ssh: SSHWorkspaceData(host: "example.com", environment: ["TERM": "screen-256color"])
        )

        try persistence.saveDevices([device])
        let reloaded = try persistence.loadDevices()

        #expect(reloaded.first?.ssh.environment["TERM"] == "screen-256color")
    }

    @Test("file persistence decodes legacy devices with default environment")
    func filePersistenceDecodesLegacyDevices() throws {
        let url = tempURL()
        try """
        [
          {
            "id": "00000000-0000-0000-0000-000000000001",
            "name": "Prod",
            "kind": "ssh",
            "ssh": {
              "host": "example.com",
              "remoteRoot": "~"
            }
          }
        ]
        """.write(to: url, atomically: true, encoding: .utf8)
        let persistence = FileRemoteDevicePersistence(fileURL: url)
        let reloaded = try persistence.loadDevices()

        #expect(reloaded.first?.ssh.environment == SSHEnvironmentVariables.default)
    }

    @Test("new devices default TERM")
    func defaultEnvironment() {
        let device = RemoteDevice(name: "Prod", ssh: SSHWorkspaceData(host: "example.com"))

        #expect(device.ssh.environment == SSHEnvironmentVariables.default)
        #expect(device.destination.environment == SSHEnvironmentVariables.default)
    }
}
