import Foundation
import Testing

@testable import Muxy

@Suite("TmuxConfiguration")
struct TmuxConfigurationTests {
    @Test("sessionName uses prefix + first 8 chars of UUID")
    func sessionName() {
        let id = UUID(uuidString: "A1B2C3D4-E5F6-7890-ABCD-EF1234567890")!
        let name = TmuxConfiguration.sessionName(for: id)
        #expect(name == "muxy-A1B2C3D4")
    }

    @Test("sessionName is consistent for the same UUID")
    func sessionNameConsistent() {
        let id = UUID()
        #expect(TmuxConfiguration.sessionName(for: id) == TmuxConfiguration.sessionName(for: id))
    }

    @Test("sessionName differs for different UUIDs")
    func sessionNameDiffers() {
        let a = UUID()
        let b = UUID()
        #expect(TmuxConfiguration.sessionName(for: a) != TmuxConfiguration.sessionName(for: b))
    }
}

@Suite("TmuxCaptureService.parseControlOutput")
struct TmuxControlOutputParsingTests {
    @Test("parses single %output line with base64 payload")
    func singleOutputLine() {
        let payload = "SGVsbG8gV29ybGQ="
        let input = "%output %1 \(payload)\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(results[0] == Data("Hello World".utf8))
    }

    @Test("parses multiple %output lines")
    func multipleOutputLines() {
        let payload1 = "YWJj"
        let payload2 = "eHl6"
        let input = "%output %1 \(payload1)\n%output %2 \(payload2)\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 2)
        #expect(results[0] == Data("abc".utf8))
        #expect(results[1] == Data("xyz".utf8))
    }

    @Test("skips non-%output lines")
    func skipsNonOutputLines() {
        let payload = "YWJj"
        let input = "%layout %1 80x24\n%output %2 \(payload)\n%exit\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(results[0] == Data("abc".utf8))
    }

    @Test("returns empty for invalid base64")
    func invalidBase64() {
        let input = "%output %1 !!!invalid!!!\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.isEmpty)
    }

    @Test("returns empty for %output with missing payload")
    func missingPayload() {
        let input = "%output %1\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.isEmpty)
    }

    @Test("returns empty for non-UTF8 input")
    func nonUTF8Input() {
        let data = Data([0xFF, 0xFE, 0xFD])
        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.isEmpty)
    }

    @Test("returns empty for empty input")
    func emptyInput() {
        let data = Data()
        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.isEmpty)
    }

    @Test("handles payload with spaces")
    func payloadWithSpaces() {
        let payload = "aGVsbG8gd29ybGQ="
        let input = "%output %1 \(payload)\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(String(data: results[0], encoding: .utf8) == "hello world")
    }
}
