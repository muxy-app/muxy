import Foundation
import Testing
@testable import Muxy

private let replayPrefix: [UInt8] = [
    0x1B, 0x5B, 0x30, 0x6D,
    0x1B, 0x28, 0x42,
    0x1B, 0x5B, 0x32, 0x4A,
    0x1B, 0x5B, 0x48,
]

private let enterAlt: [UInt8] = Array("\u{1B}[?1049h".utf8)
private let leaveAlt: [UInt8] = Array("\u{1B}[?1049l".utf8)

@Suite("Mobile scrollback buffer")
struct MobileScrollbackBufferTests {
    @Test("appends accumulate main-buffer content")
    func appendAccumulates() {
        var buffer = MobileScrollbackBuffer(capacity: 128)

        buffer.append(Array("hello".utf8), byteLimit: 128)
        buffer.append(Array(" world".utf8), byteLimit: 128)

        #expect(buffer.byteCount == 11)
        #expect(buffer.bytes == Array("hello world".utf8))
    }

    @Test("replay prepends a normalization prefix")
    func replayPrependsPrefix() {
        var buffer = MobileScrollbackBuffer(capacity: 128)
        buffer.append(Array("hello".utf8), byteLimit: 128)

        #expect(buffer.replayBytes == replayPrefix + Array("hello".utf8))
    }

    @Test("replay head jumps to a line boundary after trimming")
    func replayHeadAlignsToLine() {
        var buffer = MobileScrollbackBuffer(capacity: 5)
        buffer.append(Array("abc\ndef".utf8), byteLimit: 5)

        #expect(buffer.replayBytes == replayPrefix + Array("def".utf8))
    }

    @Test("replay head drops leading UTF-8 continuation bytes")
    func replayHeadDropsUTF8Continuation() {
        var buffer = MobileScrollbackBuffer(capacity: 4)
        buffer.append(Array("€abc".utf8), byteLimit: 4)

        #expect(buffer.replayBytes == replayPrefix + Array("abc".utf8))
    }

    @Test("alt-screen frames are dropped but markers and neighbors stay")
    func altScreenFramesAreSkipped() {
        var buffer = MobileScrollbackBuffer(capacity: 1024)
        let content = Array("before".utf8)
            + enterAlt
            + Array("f1f2f3".utf8)
            + leaveAlt
            + Array("after".utf8)

        buffer.append(content, byteLimit: 1024)

        #expect(buffer.bytes == Array("before".utf8) + enterAlt + leaveAlt + Array("after".utf8))
        #expect(!buffer.isAlternateScreenActive)
    }

    @Test("content inside an active alt screen is not captured")
    func activeAltDropsFrames() {
        var buffer = MobileScrollbackBuffer(capacity: 1024)
        buffer.append(Array("before".utf8) + enterAlt + Array("f1f2f3".utf8), byteLimit: 1024)

        #expect(buffer.isAlternateScreenActive)
        #expect(buffer.bytes == Array("before".utf8) + enterAlt)

        buffer.append(Array("more-frames".utf8), byteLimit: 1024)

        #expect(buffer.bytes == Array("before".utf8) + enterAlt)
    }

    @Test("leaving an active alt screen resumes main-buffer capture")
    func leaveAltResumesCapture() {
        var buffer = MobileScrollbackBuffer(capacity: 1024)
        buffer.append(Array("before".utf8) + enterAlt + Array("frames".utf8), byteLimit: 1024)
        buffer.append(leaveAlt + Array("after".utf8), byteLimit: 1024)

        #expect(!buffer.isAlternateScreenActive)
        #expect(buffer.bytes == Array("before".utf8) + enterAlt + leaveAlt + Array("after".utf8))
        #expect(buffer.replayBytes == replayPrefix + Array("before".utf8) + enterAlt + leaveAlt + Array("after".utf8))
    }

    @Test("alt-screen enter split across chunks is still detected once")
    func altEnterAcrossChunks() {
        var buffer = MobileScrollbackBuffer(capacity: 1024)
        let enterHead = Array("\u{1B}[?10".utf8)
        let enterTail = Array("49h".utf8)

        buffer.append(Array("before ".utf8) + enterHead, byteLimit: 1024)
        #expect(!buffer.isAlternateScreenActive)

        buffer.append(enterTail + Array("f1f2".utf8) + leaveAlt + Array("after".utf8), byteLimit: 1024)

        #expect(!buffer.isAlternateScreenActive)
        let stored = buffer.bytes
        #expect(String(decoding: stored, as: UTF8.self).contains("before"))
        #expect(String(decoding: stored, as: UTF8.self).contains("after"))
        #expect(!String(decoding: stored, as: UTF8.self).contains("f1f2"))
        #expect(String(decoding: stored, as: UTF8.self).contains("\u{1B}[?1049h"))
    }

    @Test("replay trims incomplete trailing escape and UTF-8 sequences")
    func replayTrimsIncompleteTail() {
        var buffer = MobileScrollbackBuffer(capacity: 1024)
        buffer.append(Array("abc".utf8) + Array("\u{1B}[31".utf8) + Array("한".utf8).dropLast(), byteLimit: 1024)

        #expect(String(decoding: buffer.replayBytes, as: UTF8.self) == "\u{1B}[0m\u{1B}(B\u{1B}[2J\u{1B}[Habc")
    }

    @Test("growing capacity preserves buffered bytes and alt state")
    func ensureCapacityPreservesBytes() {
        var buffer = MobileScrollbackBuffer(capacity: 4)
        buffer.append(Array("abcd".utf8), byteLimit: 4)
        #expect(buffer.bytes == Array("abcd".utf8))

        buffer.append(Array("e".utf8), byteLimit: 16)

        #expect(buffer.capacity == 16)
        #expect(buffer.bytes == Array("abcde".utf8))
    }

    @Test("trimming to a smaller cap keeps the newest bytes")
    func trimKeepsNewestBytes() {
        var buffer = MobileScrollbackBuffer(capacity: 128)
        buffer.append(Array((0 ..< 128).map { UInt8($0) }), byteLimit: 128)

        buffer.trim(toByteLimit: 32)

        #expect(buffer.byteCount == 32)
        #expect(buffer.bytes == Array((96 ..< 128).map { UInt8($0) }))
    }

    @Test("trimming shrinks the backing capacity")
    func trimShrinksCapacity() {
        var buffer = MobileScrollbackBuffer(capacity: 256)
        buffer.append(Array("content".utf8), byteLimit: 256)

        buffer.trim(toByteLimit: 8)

        #expect(buffer.capacity == 8)
        #expect(buffer.bytes == Array("content".utf8))
    }

    @Test("removeAll clears storage and alt state")
    func removeAllClears() {
        var buffer = MobileScrollbackBuffer(capacity: 1024)
        buffer.append(Array("before".utf8) + enterAlt + Array("frames".utf8), byteLimit: 1024)
        #expect(buffer.isAlternateScreenActive)

        buffer.removeAll()

        #expect(buffer.isEmpty)
        #expect(!buffer.isAlternateScreenActive)
        #expect(buffer.replayBytes.isEmpty)
    }
}
