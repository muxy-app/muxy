import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "RemoteDeviceStore")

@MainActor
@Observable
final class RemoteDeviceStore {
    private(set) var devices: [RemoteDevice] = []
    private let persistence: any RemoteDevicePersisting

    init(persistence: any RemoteDevicePersisting) {
        self.persistence = persistence
        load()
    }

    func device(id: UUID?) -> RemoteDevice? {
        guard let id else { return nil }
        return devices.first(where: { $0.id == id })
    }

    func sshDevices() -> [RemoteDevice] {
        devices.filter { $0.kind == .ssh }
    }

    @discardableResult
    func add(name: String, kind: RemoteDeviceKind = .ssh, ssh: SSHWorkspaceData) -> RemoteDevice {
        let device = RemoteDevice(name: name, kind: kind, ssh: ssh)
        devices.append(device)
        save()
        return device
    }

    func update(id: UUID, _ mutate: (inout RemoteDevice) -> Void) {
        guard let index = devices.firstIndex(where: { $0.id == id }) else { return }
        mutate(&devices[index])
        save()
    }

    func rename(id: UUID, to name: String) {
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        update(id: id) { $0.name = trimmed }
    }

    func remove(id: UUID) {
        devices.removeAll { $0.id == id }
        save()
    }

    private func save() {
        do {
            try persistence.saveDevices(devices)
        } catch {
            logger.error("Failed to save remote devices: \(error)")
        }
    }

    private func load() {
        do {
            devices = try persistence.loadDevices()
        } catch {
            logger.error("Failed to load remote devices: \(error)")
        }
    }
}
