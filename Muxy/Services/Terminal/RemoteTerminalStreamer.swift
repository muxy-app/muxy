import Foundation
import MuxyServer
import MuxyShared

@MainActor
final class RemoteTerminalStreamer {
    static let shared = RemoteTerminalStreamer()

    weak var server: MuxyRemoteServer?

    private final class Attachment {
        weak var source: (any TerminalRawOutputSource)?

        init(source: any TerminalRawOutputSource) {
            self.source = source
        }
    }

    private var attachments: [UUID: Attachment] = [:]
    private var scrollbackBuffers: [UUID: MobileScrollbackBuffer] = [:]

    private var scrollbackByteLimit: Int {
        MobileServerService.shared.scrollbackCapMB * 1_048_576
    }

    init() {}

    func attach(paneID: UUID, surface: any TerminalRawOutputSource) {
        if let existing = attachments[paneID]?.source {
            guard existing !== surface else { return }
            existing.setRawOutputHandler(nil)
        }

        surface.setRawOutputHandler { [weak self] bytes in
            self?.forward(paneID: paneID, bytes: bytes)
        }
        attachments[paneID] = Attachment(source: surface)
    }

    func detach(paneID: UUID, surface: any TerminalRawOutputSource) {
        guard attachments[paneID]?.source === surface else { return }
        surface.setRawOutputHandler(nil)
        attachments.removeValue(forKey: paneID)
    }

    private func forward(paneID: UUID, bytes: Data) {
        appendScrollback(paneID: paneID, bytes: bytes, byteLimit: scrollbackByteLimit)

        guard let clientID = PaneOwnershipStore.shared.remoteOwner(for: paneID) else { return }
        let event = MuxyEvent(
            event: .terminalOutput,
            data: .terminalOutput(TerminalOutputEventDTO(paneID: paneID, bytes: bytes))
        )
        server?.send(event, to: clientID)
    }

    func appendScrollback(paneID: UUID, bytes: Data, byteLimit: Int) {
        scrollbackBuffers[paneID, default: MobileScrollbackBuffer(capacity: 0)]
            .append(Array(bytes), byteLimit: byteLimit)
    }

    func scrollbackData(for paneID: UUID) -> Data? {
        guard let replay = scrollbackBuffers[paneID]?.replayBytes, !replay.isEmpty else { return nil }
        return Data(replay)
    }

    func trimAllScrollback(toByteLimit byteLimit: Int) {
        for paneID in scrollbackBuffers.keys {
            scrollbackBuffers[paneID]?.trim(toByteLimit: byteLimit)
        }
    }

    func resetPane(_ paneID: UUID) {
        scrollbackBuffers.removeValue(forKey: paneID)
    }
}
