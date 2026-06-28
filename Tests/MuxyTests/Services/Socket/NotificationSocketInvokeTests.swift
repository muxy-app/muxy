import Foundation
import Testing

@testable import Muxy

@Suite("NotificationSocketServer invoke-result parsing")
struct NotificationSocketInvokeTests {
    @Test("parses a successful invoke-result with base64 payload")
    func parsesOk() throws {
        let payload = Data(#"{"pong":42}"#.utf8)
        let line = "invoke-result|call-1|ok|\(payload.base64EncodedString())"
        let parsed = try #require(NotificationSocketServer.parseInvokeResult(line))
        #expect(parsed.callID == "call-1")
        #expect(parsed.ok)
        #expect(parsed.body == payload)
    }

    @Test("parses an error invoke-result carrying a message")
    func parsesError() throws {
        let message = Data("handler threw".utf8)
        let line = "invoke-result|call-2|err|\(message.base64EncodedString())"
        let parsed = try #require(NotificationSocketServer.parseInvokeResult(line))
        #expect(parsed.callID == "call-2")
        #expect(!parsed.ok)
        #expect(String(data: parsed.body, encoding: .utf8) == "handler threw")
    }

    @Test("rejects malformed invoke-result lines")
    func rejectsMalformed() {
        #expect(NotificationSocketServer.parseInvokeResult("invoke-result|call-1") == nil)
        #expect(NotificationSocketServer.parseInvokeResult("invoke-result||ok|x") == nil)
        #expect(NotificationSocketServer.parseInvokeResult("invoke-result|call-1|maybe|x") == nil)
        #expect(NotificationSocketServer.parseInvokeResult("event|call-1|ok|x") == nil)
    }
}
