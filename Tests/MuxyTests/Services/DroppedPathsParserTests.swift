import Foundation
import Testing

@testable import Muxy

@Suite("DroppedPathsParser")
struct DroppedPathsParserTests {
    @Test("file URLs are returned as filesystem paths")
    func fileURLs() {
        let urls = [URL(fileURLWithPath: "/tmp/a.txt"), URL(fileURLWithPath: "/tmp/b.txt")]
        #expect(DroppedPathsParser.parse(fileURLs: urls, plainString: nil) == ["/tmp/a.txt", "/tmp/b.txt"])
    }

    @Test("non-file URLs are filtered out")
    func nonFileURLs() {
        let urls = [URL(string: "https://example.com")!, URL(fileURLWithPath: "/tmp/a.txt")]
        #expect(DroppedPathsParser.parse(fileURLs: urls, plainString: nil) == ["/tmp/a.txt"])
    }

    @Test("empty inputs return empty")
    func empty() {
        #expect(DroppedPathsParser.parse(fileURLs: [], plainString: nil).isEmpty)
        #expect(DroppedPathsParser.parse(fileURLs: [], plainString: "").isEmpty)
    }

    @Test("file:// strings are decoded to paths")
    func fileURLString() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "file:///tmp/a.txt",
            fileExists: { _ in false }
        )
        #expect(result == ["/tmp/a.txt"])
    }

    @Test("absolute paths that exist on disk are accepted")
    func absolutePathExists() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/a.txt",
            fileExists: { $0 == "/tmp/a.txt" }
        )
        #expect(result == ["/tmp/a.txt"])
    }

    @Test("absolute paths that do not exist are rejected as a whole drop")
    func absolutePathMissing() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/missing.txt",
            fileExists: { _ in false }
        )
        #expect(result.isEmpty)
    }

    @Test("mixed valid and invalid lines reject the entire drop")
    func mixedRejected() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/a.txt\nrandom log line\n/tmp/b.txt",
            fileExists: { _ in true }
        )
        #expect(result.isEmpty)
    }

    @Test("multiple valid lines all accepted")
    func multipleValid() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "/tmp/a.txt\nfile:///tmp/b.txt",
            fileExists: { _ in true }
        )
        #expect(result == ["/tmp/a.txt", "/tmp/b.txt"])
    }

    @Test("file URLs take precedence over plain string")
    func urlsPrecedence() {
        let result = DroppedPathsParser.parse(
            fileURLs: [URL(fileURLWithPath: "/tmp/a.txt")],
            plainString: "/tmp/b.txt",
            fileExists: { _ in true }
        )
        #expect(result == ["/tmp/a.txt"])
    }

    @Test("non-path text is rejected")
    func nonPathText() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "hello world",
            fileExists: { _ in true }
        )
        #expect(result.isEmpty)
    }

    @Test("whitespace around lines is trimmed")
    func trimmed() {
        let result = DroppedPathsParser.parse(
            fileURLs: [],
            plainString: "   /tmp/a.txt  \n  /tmp/b.txt  ",
            fileExists: { _ in true }
        )
        #expect(result == ["/tmp/a.txt", "/tmp/b.txt"])
    }
}
