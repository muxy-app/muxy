import Foundation

protocol ApprovedDevicesPersisting {
    func loadDevices() throws -> [ApprovedDevice]
    func saveDevices(_ devices: [ApprovedDevice]) throws
}

final class FileApprovedDevicesPersistence: ApprovedDevicesPersisting {
    private let store: CodableFileStore<[ApprovedDevice]>

    init(fileURL: URL = MuxyFileStorage.fileURL(filename: "approved-devices.json")) {
        store = CodableFileStore(
            fileURL: fileURL,
            options: CodableFileStoreOptions(filePermissions: FilePermissions.privateFile)
        )
    }

    func loadDevices() throws -> [ApprovedDevice] {
        try store.load() ?? []
    }

    func saveDevices(_ devices: [ApprovedDevice]) throws {
        try store.save(devices)
    }
}
