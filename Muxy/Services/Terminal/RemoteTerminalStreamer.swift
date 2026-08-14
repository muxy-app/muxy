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
    private var scrollbackBuffers: [UUID: Data] = [:]

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
        scrollbackBuffers[paneID, default: Data()].append(bytes)
        guard let count = scrollbackBuffers[paneID]?.count, count > byteLimit else { return }
        let trimTarget = max(byteLimit * 3 / 4, 1)
        scrollbackBuffers[paneID]?.removeFirst(count - trimTarget)
    }

    func scrollbackData(for paneID: UUID) -> Data? {
        scrollbackBuffers[paneID]
    }

    func trimAllScrollback(toByteLimit byteLimit: Int) {
        let trimTarget = max(byteLimit * 3 / 4, 1)
        for paneID in scrollbackBuffers.keys {
            guard let count = scrollbackBuffers[paneID]?.count, count > byteLimit else { continue }
            scrollbackBuffers[paneID]?.removeFirst(count - trimTarget)
        }
    }

    func resetPane(_ paneID: UUID) {
        scrollbackBuffers.removeValue(forKey: paneID)
    }
}
