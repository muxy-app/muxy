import Foundation
import Testing

@testable import Muxy

@Suite("NotificationSocketServer command reply framing")
struct NotificationSocketCommandReplyTests {
    @Test("CLI reply is terminated so empty responses are distinguishable from no response")
    func cliReplyIsTerminated() {
        let empty = NotificationSocketServer.framedCommandReply(response: "", isExtensionSession: false)
        #expect(empty == Data([NotificationSocketServer.commandReplyTerminator]))

        let content = NotificationSocketServer.framedCommandReply(response: "hello", isExtensionSession: false)
        #expect(content.last == NotificationSocketServer.commandReplyTerminator)
        #expect(content.dropLast() == Data("hello".utf8))
    }

    @Test("extension reply stays newline-delimited without the terminator")
    func extensionReplyIsNewlineDelimited() {
        let reply = NotificationSocketServer.framedCommandReply(response: "ok", isExtensionSession: true)
        #expect(reply == Data("ok\n".utf8))
        #expect(!reply.contains(NotificationSocketServer.commandReplyTerminator))
    }

    @Test("CLI terminator never collides with text payloads")
    func terminatorIsNul() {
        #expect(NotificationSocketServer.commandReplyTerminator == 0)
    }
}
