import Foundation
import MuxyShared
import SystemConfiguration

@MainActor
@Observable
final class PaneOwnershipStore {
    static let shared = PaneOwnershipStore()

    var macDeviceName: String = {
        if let name = SCDynamicStoreCopyComputerName(nil, nil) as String?, !name.isEmpty {
            return name
        }
        return "Mac"
    }()

    private var owners: [UUID: PaneOwnerDTO] = [:]
    private var deviceNames: [UUID: String] = [:]
    private var ownedPanesByClient: [UUID: Set<UUID>] = [:]

    var onOwnershipChanged: ((UUID, PaneOwnerDTO) -> Void)?

    private init() {}

    func owner(for paneID: UUID) -> PaneOwnerDTO {
        owners[paneID] ?? .mac(deviceName: macDeviceName)
    }

    func isOwnedByMac(_ paneID: UUID) -> Bool {
        if case .mac = owner(for: paneID) { return true }
        return false
    }

    func isOwnedBy(clientID: UUID, paneID: UUID) -> Bool {
        if case let .remote(id, _) = owner(for: paneID), id == clientID { return true }
        return false
    }

    func remoteOwner(for paneID: UUID) -> UUID? {
        if case let .remote(clientID, _) = owners[paneID] { return clientID }
        return nil
    }

    func registerDevice(clientID: UUID, name: String) {
        deviceNames[clientID] = name
    }

    func assign(paneID: UUID, to clientID: UUID) {
        let name = deviceNames[clientID] ?? "Mobile"
        if case let .remote(existing, _) = owners[paneID], existing != clientID {
            ownedPanesByClient[existing]?.remove(paneID)
        }
        let newOwner = PaneOwnerDTO.remote(deviceID: clientID, deviceName: name)
        owners[paneID] = newOwner
        ownedPanesByClient[clientID, default: []].insert(paneID)
        onOwnershipChanged?(paneID, newOwner)
    }

    func releaseToMac(paneID: UUID) {
        if case let .remote(clientID, _) = owners[paneID] {
            ownedPanesByClient[clientID]?.remove(paneID)
        }
        owners[paneID] = nil
        let owner = PaneOwnerDTO.mac(deviceName: macDeviceName)
        onOwnershipChanged?(paneID, owner)
    }

    func releaseAll(clientID: UUID) {
        guard let panes = ownedPanesByClient.removeValue(forKey: clientID) else { return }
        for paneID in panes {
            owners[paneID] = nil
            let owner = PaneOwnerDTO.mac(deviceName: macDeviceName)
            onOwnershipChanged?(paneID, owner)
        }
        deviceNames.removeValue(forKey: clientID)
    }
}
