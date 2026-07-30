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

    func attach(
        paneID: UUID,
        surface: ghostty_surface_t,
        shellActivityTracker: TerminalShellActivityTracker
    ) {
        if tokenByPane[paneID] != nil {
            return
        }
        let token = nextToken
        nextToken += 1
        tokenByPane[paneID] = token
        paneByToken[token] = paneID
        TerminalShellActivityRegistry.shared.register(shellActivityTracker.beginSession(), for: token)
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
            TerminalShellActivityRegistry.shared.removeSession(for: token)?.invalidate()
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

private final class TerminalShellActivityRegistry: @unchecked Sendable {
    static let shared = TerminalShellActivityRegistry()

    private let lock = NSLock()
    private var sessions: [Int: TerminalShellActivityTracker.Session] = [:]

    private init() {}

    func register(_ session: TerminalShellActivityTracker.Session, for token: Int) {
        lock.withLock {
            sessions[token] = session
        }
    }

    func session(for token: Int) -> TerminalShellActivityTracker.Session? {
        lock.withLock { sessions[token] }
    }

    func removeSession(for token: Int) -> TerminalShellActivityTracker.Session? {
        lock.withLock {
            sessions.removeValue(forKey: token)
        }
    }
}

private let ptyDataCallback: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, UInt) -> Void = { userdata, ptr, len in
    guard let userdata,
          let ptr,
          len > 0
    else { return }
    let token = Int(bitPattern: userdata)
    let bytes = Data(bytes: ptr, count: Int(len))
    TerminalShellActivityRegistry.shared.session(for: token)?.recordOutput(bytes)
    DispatchQueue.main.async {
        MainActor.assumeIsolated {
            guard let paneID = RemoteTerminalStreamer.shared.pane(for: token) else { return }
            RemoteTerminalStreamer.shared.forward(paneID: paneID, bytes: bytes)
        }
    }
}
