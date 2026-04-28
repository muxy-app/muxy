import CryptoKit
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ApprovedDevicesStore")

struct ApprovedDevice: Codable, Identifiable, Equatable {
    let id: UUID
    var name: String
    let tokenHash: String
    let approvedAt: Date
    var lastSeenAt: Date?
}

@MainActor
@Observable
final class ApprovedDevicesStore {
    static let shared = ApprovedDevicesStore()

    private static let store = CodableFileStore<[ApprovedDevice]>(
        fileURL: MuxyFileStorage.fileURL(filename: "approved-devices.json"),
        options: CodableFileStoreOptions(filePermissions: FilePermissions.privateFile)
    )

    private(set) var devices: [ApprovedDevice] = []

    var onRevoke: ((UUID) -> Void)?

    private init() {
        do {
            devices = try Self.store.load() ?? []
        } catch {
            logger.error("Failed to load approved devices: \(error)")
        }
    }

    func approve(deviceID: UUID, name: String, token: String) {
        let hash = Self.hash(token)
        let now = Date()
        if let index = devices.firstIndex(where: { $0.id == deviceID }) {
            devices[index] = ApprovedDevice(
                id: deviceID,
                name: name,
                tokenHash: hash,
                approvedAt: devices[index].approvedAt,
                lastSeenAt: now
            )
        } else {
            devices.append(ApprovedDevice(
                id: deviceID,
                name: name,
                tokenHash: hash,
                approvedAt: now,
                lastSeenAt: now
            ))
        }
        save()
    }

    func validate(deviceID: UUID, token: String) -> ApprovedDevice? {
        guard let device = devices.first(where: { $0.id == deviceID }) else { return nil }
        let provided = Self.hash(token)
        guard Self.constantTimeEquals(device.tokenHash, provided) else { return nil }
        return device
    }

    func touch(deviceID: UUID) {
        guard let index = devices.firstIndex(where: { $0.id == deviceID }) else { return }
        devices[index].lastSeenAt = Date()
        save()
    }

    func rename(deviceID: UUID, to newName: String) {
        guard let index = devices.firstIndex(where: { $0.id == deviceID }) else { return }
        devices[index].name = newName
        save()
    }

    func revoke(deviceID: UUID) {
        devices.removeAll { $0.id == deviceID }
        save()
        onRevoke?(deviceID)
    }

    static func hash(_ token: String) -> String {
        let digest = SHA256.hash(data: Data(token.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static func constantTimeEquals(_ lhs: String, _ rhs: String) -> Bool {
        let left = Array(lhs.utf8)
        let right = Array(rhs.utf8)
        guard left.count == right.count else { return false }
        var diff: UInt8 = 0
        for index in left.indices {
            diff |= left[index] ^ right[index]
        }
        return diff == 0
    }

    private func save() {
        do {
            try Self.store.save(devices)
        } catch {
            logger.error("Failed to save approved devices: \(error)")
        }
    }
}
