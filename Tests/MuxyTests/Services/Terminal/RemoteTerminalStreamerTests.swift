import Foundation
import Testing
@testable import Muxy

@Suite("Remote terminal streamer")
@MainActor
struct RemoteTerminalStreamerTests {
    @Test("attach installs one raw output handler")
    func attachInstallsHandler() {
        let streamer = RemoteTerminalStreamer()
        let surface = TerminalRawOutputTestSource()

        streamer.attach(paneID: UUID(), surface: surface)

        #expect(surface.handlerInstallCount == 1)
        #expect(surface.hasHandler)
    }

    @Test("duplicate attach keeps the existing handler")
    func duplicateAttachIsIdempotent() {
        let streamer = RemoteTerminalStreamer()
        let surface = TerminalRawOutputTestSource()
        let paneID = UUID()

        streamer.attach(paneID: paneID, surface: surface)
        streamer.attach(paneID: paneID, surface: surface)

        #expect(surface.handlerInstallCount == 1)
        #expect(surface.handlerRemovalCount == 0)
    }

    @Test("detach removes the active handler once")
    func detachRemovesHandler() {
        let streamer = RemoteTerminalStreamer()
        let surface = TerminalRawOutputTestSource()
        let paneID = UUID()

        streamer.attach(paneID: paneID, surface: surface)
        streamer.detach(paneID: paneID, surface: surface)
        streamer.detach(paneID: paneID, surface: surface)

        #expect(!surface.hasHandler)
        #expect(surface.handlerRemovalCount == 1)
    }

    @Test("recreated surface replaces the previous handler")
    func recreatedSurfaceReplacesHandler() {
        let streamer = RemoteTerminalStreamer()
        let previous = TerminalRawOutputTestSource()
        let recreated = TerminalRawOutputTestSource()
        let paneID = UUID()

        streamer.attach(paneID: paneID, surface: previous)
        streamer.attach(paneID: paneID, surface: recreated)
        streamer.detach(paneID: paneID, surface: previous)

        #expect(!previous.hasHandler)
        #expect(previous.handlerRemovalCount == 1)
        #expect(recreated.hasHandler)
        #expect(recreated.handlerInstallCount == 1)
        #expect(recreated.handlerRemovalCount == 0)
    }
}

@MainActor
private final class TerminalRawOutputTestSource: TerminalRawOutputSource {
    private var handler: ((Data) -> Void)?
    private(set) var handlerInstallCount = 0
    private(set) var handlerRemovalCount = 0

    var hasHandler: Bool {
        handler != nil
    }

    func setRawOutputHandler(_ handler: ((Data) -> Void)?) {
        self.handler = handler
        if handler == nil {
            handlerRemovalCount += 1
        } else {
            handlerInstallCount += 1
        }
    }
}
