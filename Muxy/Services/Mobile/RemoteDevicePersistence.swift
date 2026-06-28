import Foundation

protocol RemoteDevicePersisting {
    func loadDevices() throws -> [RemoteDevice]
    func saveDevices(_ devices: [RemoteDevice]) throws
}

final class FileRemoteDevicePersistence: RemoteDevicePersisting {
    private let store: CodableFileStore<[RemoteDevice]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "remote-devices.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(filePermissions: FilePermissions.privateFile)
        )
    }

    func loadDevices() throws -> [RemoteDevice] {
        try store.load() ?? []
    }

    func saveDevices(_ devices: [RemoteDevice]) throws {
        try store.save(devices)
    }
}

final class InMemoryRemoteDevicePersistence: RemoteDevicePersisting {
    private var devices: [RemoteDevice]

    init(initial: [RemoteDevice] = []) {
        devices = initial
    }

    func loadDevices() throws -> [RemoteDevice] {
        devices
    }

    func saveDevices(_ devices: [RemoteDevice]) throws {
        self.devices = devices
    }
}
