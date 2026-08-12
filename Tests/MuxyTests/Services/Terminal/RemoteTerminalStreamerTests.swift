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

    @Test("appendScrollback accumulates output")
    func appendScrollbackAccumulates() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()

        streamer.appendScrollback(paneID: paneID, bytes: Data("hello".utf8), byteLimit: 1024)
        streamer.appendScrollback(paneID: paneID, bytes: Data(" world".utf8), byteLimit: 1024)

        #expect(streamer.scrollbackData(for: paneID) == Data("hello world".utf8))
    }

    @Test("appendScrollback trims oldest bytes when over the limit")
    func appendScrollbackTrimsToLimit() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()

        streamer.appendScrollback(paneID: paneID, bytes: Data("abcdef".utf8), byteLimit: 5)

        #expect(streamer.scrollbackData(for: paneID) == Data("def".utf8))
    }

    @Test("appendScrollback keeps the buffer within the limit across repeated appends")
    func appendScrollbackBoundsBuffer() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()
        let byteLimit = 10

        for _ in 0..<20 {
            streamer.appendScrollback(paneID: paneID, bytes: Data("abcdefghij".utf8), byteLimit: byteLimit)
        }

        #expect((streamer.scrollbackData(for: paneID)?.count ?? 0) <= byteLimit)
    }

    @Test("scrollbackData does not consume the buffer")
    func scrollbackDataIsNonDestructive() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()
        let bytes = Data("hello".utf8)

        streamer.appendScrollback(paneID: paneID, bytes: bytes, byteLimit: 1024)

        #expect(streamer.scrollbackData(for: paneID) == bytes)
        #expect(streamer.scrollbackData(for: paneID) == bytes)
    }

    @Test("forward buffers output from the surface handler")
    func forwardBuffersOutput() {
        let streamer = RemoteTerminalStreamer()
        let surface = TerminalRawOutputTestSource()
        let paneID = UUID()

        streamer.attach(paneID: paneID, surface: surface)
        surface.fire(Data("hello".utf8))
        surface.fire(Data(" world".utf8))

        #expect(streamer.scrollbackData(for: paneID) == Data("hello world".utf8))
        streamer.detach(paneID: paneID, surface: surface)
    }

    @Test("isAltBuffer defaults to false for an unknown pane")
    func isAltBufferDefaultsFalse() {
        let streamer = RemoteTerminalStreamer()

        #expect(!streamer.isAltBuffer(for: UUID()))
    }

    @Test("setAltBuffer sets the tracked buffer mode")
    func setAltBufferSetsState() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()

        streamer.setAltBuffer(for: paneID, active: true)
        #expect(streamer.isAltBuffer(for: paneID))

        streamer.setAltBuffer(for: paneID, active: false)
        #expect(!streamer.isAltBuffer(for: paneID))
    }

    @Test("forward detects alt buffer entry from the output stream")
    func forwardDetectsAltBufferEnable() {
        let streamer = RemoteTerminalStreamer()
        let surface = TerminalRawOutputTestSource()
        let paneID = UUID()

        streamer.attach(paneID: paneID, surface: surface)
        surface.fire(Data("hello\u{1B}[?1049h".utf8))

        #expect(streamer.isAltBuffer(for: paneID))
        streamer.detach(paneID: paneID, surface: surface)
    }

    @Test("forward detects alt buffer exit from the output stream")
    func forwardDetectsAltBufferDisable() {
        let streamer = RemoteTerminalStreamer()
        let surface = TerminalRawOutputTestSource()
        let paneID = UUID()

        streamer.attach(paneID: paneID, surface: surface)
        surface.fire(Data("\u{1B}[?1049h".utf8))
        surface.fire(Data("\u{1B}[?1049l".utf8))

        #expect(!streamer.isAltBuffer(for: paneID))
        streamer.detach(paneID: paneID, surface: surface)
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

    func fire(_ bytes: Data) {
        handler?(bytes)
    }
}
