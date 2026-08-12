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
    private var altBufferActive: [UUID: Bool] = [:]

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
        trackAltBuffer(paneID: paneID, bytes: bytes)

        guard let clientID = PaneOwnershipStore.shared.remoteOwner(for: paneID) else { return }
        let event = MuxyEvent(
            event: .terminalOutput,
            data: .terminalOutput(TerminalOutputEventDTO(paneID: paneID, bytes: bytes))
        )
        server?.send(event, to: clientID)
    }

    private func trackAltBuffer(paneID: UUID, bytes: Data) {
        guard let text = String(data: bytes, encoding: .utf8) else { return }
        if text.contains("\u{1B}[?1049h") || text.contains("\u{1B}[?47h") {
            altBufferActive[paneID] = true
        } else if text.contains("\u{1B}[?1049l") || text.contains("\u{1B}[?47l") {
            altBufferActive[paneID] = false
        }
    }

    func isAltBuffer(for paneID: UUID) -> Bool {
        altBufferActive[paneID] ?? false
    }

    func setAltBuffer(for paneID: UUID, active: Bool) {
        altBufferActive[paneID] = active
    }

    func appendScrollback(paneID: UUID, bytes: Data, byteLimit: Int) {
        var buffer = scrollbackBuffers[paneID] ?? Data()
        buffer.append(bytes)
        if buffer.count > byteLimit {
            let trimTarget = max(byteLimit * 3 / 4, 1)
            buffer.removeFirst(buffer.count - trimTarget)
        }
        scrollbackBuffers[paneID] = buffer
    }

    func scrollbackData(for paneID: UUID) -> Data? {
        scrollbackBuffers[paneID]
    }
}
