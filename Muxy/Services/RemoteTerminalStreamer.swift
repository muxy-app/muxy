import Foundation
import GhosttyKit
import MuxyServer
import MuxyShared

@MainActor
final class RemoteTerminalStreamer {
    static let shared = RemoteTerminalStreamer()

    weak var server: MuxyRemoteServer?

    private var paneByToken: [Int: UUID] = [:]
    private var tokenByPane: [UUID: Int] = [:]
    private var nextToken: Int = 1

    private init() {}

    func attach(paneID: UUID, surface: ghostty_surface_t) {
        if tokenByPane[paneID] != nil { return }
        let token = nextToken
        nextToken += 1
        tokenByPane[paneID] = token
        paneByToken[token] = paneID
        ghostty_surface_set_data_callback(
            surface,
            ptyDataCallback,
            UnsafeMutableRawPointer(bitPattern: UInt(token))
        )
    }

    func detach(paneID: UUID, surface: ghostty_surface_t) {
        ghostty_surface_set_data_callback(surface, nil, nil)
        if let token = tokenByPane.removeValue(forKey: paneID) {
            paneByToken.removeValue(forKey: token)
        }
    }

    fileprivate func pane(for token: Int) -> UUID? {
        paneByToken[token]
    }

    fileprivate func forward(paneID: UUID, bytes: Data) {
        guard let clientID = PaneOwnershipStore.shared.remoteOwner(for: paneID) else { return }
        let event = MuxyEvent(
            event: .terminalOutput,
            data: .terminalOutput(TerminalOutputEventDTO(paneID: paneID, bytes: bytes))
        )
        server?.send(event, to: clientID)
    }
}

private let ptyDataCallback: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt) -> Void = { userdata, ptr, len in
    guard let userdata,
          let ptr,
          len > 0
    else { return }
    let token = Int(bitPattern: userdata)
    let bytes = Data(bytes: ptr, count: Int(len))
    DispatchQueue.main.async {
        MainActor.assumeIsolated {
            guard let paneID = RemoteTerminalStreamer.shared.pane(for: token) else { return }
            RemoteTerminalStreamer.shared.forward(paneID: paneID, bytes: bytes)
        }
    }
}
