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

    @Test("resetPane releases the buffer for a removed pane")
    func resetPaneReleasesBuffer() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()

        streamer.appendScrollback(paneID: paneID, bytes: Data("hello".utf8), byteLimit: 1024)
        streamer.resetPane(paneID)

        #expect(streamer.scrollbackData(for: paneID) == nil)
    }

    @Test("resetPane leaves other panes untouched")
    func resetPaneIsScopedToOnePane() {
        let streamer = RemoteTerminalStreamer()
        let removed = UUID()
        let kept = UUID()

        streamer.appendScrollback(paneID: removed, bytes: Data("gone".utf8), byteLimit: 1024)
        streamer.appendScrollback(paneID: kept, bytes: Data("stays".utf8), byteLimit: 1024)
        streamer.resetPane(removed)

        #expect(streamer.scrollbackData(for: kept) == Data("stays".utf8))
    }

    @Test("detach preserves the buffer so a later takeover can replay it")
    func detachPreservesBuffer() {
        let streamer = RemoteTerminalStreamer()
        let surface = TerminalRawOutputTestSource()
        let paneID = UUID()

        streamer.attach(paneID: paneID, surface: surface)
        surface.fire(Data("history".utf8))
        streamer.detach(paneID: paneID, surface: surface)

        #expect(streamer.scrollbackData(for: paneID) == Data("history".utf8))
    }

    @Test("appendScrollback keeps the newest bytes after trimming")
    func appendScrollbackKeepsNewestBytes() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()
        let byteLimit = 8

        streamer.appendScrollback(paneID: paneID, bytes: Data("abcdefgh".utf8), byteLimit: byteLimit)
        streamer.appendScrollback(paneID: paneID, bytes: Data("ij".utf8), byteLimit: byteLimit)

        let buffer = streamer.scrollbackData(for: paneID)
        #expect(buffer?.count ?? 0 <= byteLimit)
        #expect(buffer?.suffix(2) == Data("ij".utf8))
    }

    @Test("trimAllScrollback shrinks oversized buffers to the new limit")
    func trimAllScrollbackShrinksOversizedBuffers() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()
        let bytes = Data((0 ..< 128).map { UInt8($0) })

        streamer.appendScrollback(paneID: paneID, bytes: bytes, byteLimit: 256)

        streamer.trimAllScrollback(toByteLimit: 64)

        let buffer = streamer.scrollbackData(for: paneID)
        #expect((buffer?.count ?? 0) <= 64)
        #expect(buffer?.suffix(48) == bytes.suffix(48))
    }

    @Test("trimAllScrollback leaves buffers at or under the limit untouched")
    func trimAllScrollbackLeavesSmallBuffers() {
        let streamer = RemoteTerminalStreamer()
        let paneID = UUID()
        let bytes = Data("small".utf8)

        streamer.appendScrollback(paneID: paneID, bytes: bytes, byteLimit: 256)

        streamer.trimAllScrollback(toByteLimit: 64)

        #expect(streamer.scrollbackData(for: paneID) == bytes)
    }

    @Test("trimAllScrollback trims every pane independently")
    func trimAllScrollbackTrimsEveryPane() {
        let streamer = RemoteTerminalStreamer()
        let oversized = UUID()
        let small = UUID()
        let oversizedBytes = Data((0 ..< 128).map { UInt8($0) })

        streamer.appendScrollback(paneID: oversized, bytes: oversizedBytes, byteLimit: 256)
        streamer.appendScrollback(paneID: small, bytes: Data("keep".utf8), byteLimit: 256)

        streamer.trimAllScrollback(toByteLimit: 64)

        let buffer = streamer.scrollbackData(for: oversized)
        #expect((buffer?.count ?? 0) <= 64)
        #expect(buffer?.suffix(48) == oversizedBytes.suffix(48))
        #expect(streamer.scrollbackData(for: small) == Data("keep".utf8))
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
