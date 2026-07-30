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
        guard let clientID = PaneOwnershipStore.shared.remoteOwner(for: paneID) else { return }
        let event = MuxyEvent(
            event: .terminalOutput,
            data: .terminalOutput(TerminalOutputEventDTO(paneID: paneID, bytes: bytes))
        )
        server?.send(event, to: clientID)
    }
}
