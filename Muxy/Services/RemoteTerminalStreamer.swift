import Foundation
import GhosttyKit
import MuxyServer
import MuxyShared
import os

private let logger = Logger(subsystem: "app.muxy", category: "RemoteTerminalStreamer")

@MainActor
final class RemoteTerminalStreamer {
    static let shared = RemoteTerminalStreamer()

    weak var server: MuxyRemoteServer?

    private var contexts: [UUID: UnsafeMutablePointer<PaneID>] = [:]

    private init() {}

    func attach(paneID: UUID, surface: ghostty_surface_t) {
        if contexts[paneID] != nil { return }
        let context = UnsafeMutablePointer<PaneID>.allocate(capacity: 1)
        context.initialize(to: PaneID(value: paneID))
        contexts[paneID] = context
        ghostty_surface_set_data_callback(surface, ptyDataCallback, UnsafeMutableRawPointer(context))
    }

    func detach(paneID: UUID, surface: ghostty_surface_t) {
        ghostty_surface_set_data_callback(surface, nil, nil)
        if let context = contexts.removeValue(forKey: paneID) {
            context.deinitialize(count: 1)
            context.deallocate()
        }
    }

    fileprivate func receive(paneID: UUID, bytes: Data) {
        guard let clientID = PaneOwnershipStore.shared.remoteOwner(for: paneID) else { return }
        let event = MuxyEvent(
            event: .terminalOutput,
            data: .terminalOutput(TerminalOutputEventDTO(paneID: paneID, bytes: bytes))
        )
        server?.send(event, to: clientID)
    }
}

private final class PaneID {
    let value: UUID
    init(value: UUID) {
        self.value = value
    }
}

private let ptyDataCallback: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt) -> Void = { userdata, ptr, len in
    guard let userdata,
          let ptr,
          len > 0
    else { return }
    let paneID = userdata.assumingMemoryBound(to: PaneID.self).pointee.value
    let bytes = Data(bytes: ptr, count: Int(len))
    DispatchQueue.main.async {
        MainActor.assumeIsolated {
            RemoteTerminalStreamer.shared.receive(paneID: paneID, bytes: bytes)
        }
    }
}
