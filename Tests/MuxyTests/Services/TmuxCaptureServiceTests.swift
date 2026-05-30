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
    @Test("parses single %output line with plain text")
    func singlePlainOutput() {
        let input = "%output %1 Hello World\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(results[0] == Data("Hello World".utf8))
    }

    @Test("parses %output with octal-escaped carriage return and newline")
    func octalEscapedOutput() {
        let input = "%output %0 Hello\015\012World\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        let expected = Data("Hello\r\nWorld".utf8)
        #expect(results[0] == expected)
    }

    @Test("parses %output with ANSI escape sequence")
    func ansiEscapeOutput() {
        let input = "%output %0 \033[31mRed\033[0m\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        let expected = Data("\u{1B}[31mRed\u{1B}[0m".utf8)
        #expect(results[0] == expected)
    }

    @Test("parses multiple %output lines")
    func multipleOutputLines() {
        let input = "%output %1 abc\015\012\n%output %2 xyz\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 2)
        #expect(results[0] == Data("abc\r\n".utf8))
        #expect(results[1] == Data("xyz".utf8))
    }

    @Test("skips non-%output lines")
    func skipsNonOutputLines() {
        let input = "%layout %1 80x24\n%output %2 data\n%exit\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(results[0] == Data("data".utf8))
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
        let input = "%output %1 hello world\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(String(data: results[0], encoding: .utf8) == "hello world")
    }

    @Test("handles octal escape for null byte")
    func octalNullByte() {
        let input = "%output %0 a\000b\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        var expected = Data("a".utf8)
        expected.append(0x00)
        expected.append(Data("b".utf8))
        #expect(results[0] == expected)
    }

    @Test("handles multiple consecutive octal escapes")
    func consecutiveOctalEscapes() {
        let input = "%output %0 \015\012\033[31m\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        let expected = Data("\r\n\u{1B}[31m".utf8)
        #expect(results[0] == expected)
    }

    @Test("handles backslash followed by non-octal as literal backslash")
    func backslashNonOctal() {
        let input = "%output %0 path\\to\\file\n"
        let data = input.data(using: .utf8)!

        let results = TmuxCaptureService.parseControlOutput(data)
        #expect(results.count == 1)
        #expect(String(data: results[0], encoding: .utf8) == "path\\to\\file")
    }
}
