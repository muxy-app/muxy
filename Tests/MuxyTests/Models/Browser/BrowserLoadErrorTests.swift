import Foundation
import Testing

@testable import Muxy

@Suite("BrowserLoadError")
struct BrowserLoadErrorTests {
    @Test("cancelled navigation produces no error")
    func cancelledIsIgnored() {
        let error = NSError(domain: NSURLErrorDomain, code: NSURLErrorCancelled)
        #expect(BrowserLoadError.make(from: error, url: URL(string: "https://muxy.app")) == nil)
    }

    @Test("host not found produces an error with url and message")
    func hostNotFound() {
        let error = NSError(
            domain: NSURLErrorDomain,
            code: NSURLErrorCannotFindHost,
            userInfo: [NSLocalizedDescriptionKey: "A server with the specified hostname could not be found."]
        )
        let result = BrowserLoadError.make(from: error, url: URL(string: "https://invalid.test"))
        #expect(result?.failedURL == "https://invalid.test")
        #expect(result?.message == "A server with the specified hostname could not be found.")
    }

    @Test("missing url falls back to empty string")
    func missingURL() {
        let error = NSError(domain: NSURLErrorDomain, code: NSURLErrorTimedOut)
        let result = BrowserLoadError.make(from: error, url: nil)
        #expect(result?.failedURL == "")
    }
}
