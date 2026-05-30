import Darwin
import Foundation
import Testing

@testable import MuxyExtensionHost

@Suite("MuxyExtensionHost event parsing")
struct ExtensionHostBridgeTests {
    @Test("parses an event line with payload key/value pairs")
    func parsesEventWithPayload() {
        let parsed = HostBridge.parseEvent("event|pane.created|paneID=abc|title=hello")
        let result = try? #require(parsed)
        #expect(result?.name == "pane.created")
        #expect(result?.payload["paneID"] == "abc")
        #expect(result?.payload["title"] == "hello")
    }

    @Test("parses an event line without payload")
    func parsesBareEvent() {
        let parsed = HostBridge.parseEvent("event|tab.focused")
        #expect(parsed?.name == "tab.focused")
        #expect(parsed?.payload.isEmpty == true)
    }

    @Test("keeps equals signs inside a payload value")
    func keepsEqualsInValue() {
        let parsed = HostBridge.parseEvent("event|notification.posted|body=a=b=c")
        #expect(parsed?.payload["body"] == "a=b=c")
    }

    @Test("rejects non-event lines")
    func rejectsNonEvent() {
        #expect(HostBridge.parseEvent("ok") == nil)
        #expect(HostBridge.parseEvent("error:nope") == nil)
        #expect(HostBridge.parseEvent("event|") == nil)
    }

    @Test("payloadJSON produces valid JSON object")
    func payloadJSONRoundTrips() throws {
        let json = HostBridge.payloadJSON(["paneID": "abc", "title": "hi"])
        let data = Data(json.utf8)
        let object = try JSONSerialization.jsonObject(with: data) as? [String: String]
        #expect(object?["paneID"] == "abc")
        #expect(object?["title"] == "hi")
    }
}

@Suite("MuxyExtensionHost socket client")
struct ExtensionHostSocketClientTests {
    private func makePair() -> (client: HostSocketClient, server: Int32) {
        var fds: [Int32] = [0, 0]
        _ = socketpair(AF_UNIX, SOCK_STREAM, 0, &fds)
        return (HostSocketClient(fileDescriptor: fds[0]), fds[1])
    }

    private func writeLine(_ line: String, to fd: Int32) {
        let data = Data((line + "\n").utf8)
        data.withUnsafeBytes { _ = Darwin.write(fd, $0.baseAddress, data.count) }
    }

    @Test("a reply unblocks sendAndWaitReply")
    func replyUnblocks() throws {
        let (client, server) = makePair()
        defer { close(server) }
        client.startReading()

        Thread.detachNewThread { [server] in
            var buffer = [UInt8](repeating: 0, count: 256)
            _ = read(server, &buffer, buffer.count)
            self.writeLine("ok", to: server)
        }

        let reply = try client.sendAndWaitReply("identify|demo|token")
        #expect(reply == "ok")
    }

    @Test("an interleaved event line is routed to onEvent and does not satisfy a reply")
    func eventDoesNotSatisfyReply() throws {
        let (client, server) = makePair()
        defer { close(server) }

        let received = EventBox()
        client.onEvent { received.append($0) }
        client.startReading()

        Thread.detachNewThread { [server] in
            var buffer = [UInt8](repeating: 0, count: 256)
            _ = read(server, &buffer, buffer.count)
            self.writeLine("event|pane.created|paneID=abc", to: server)
            self.writeLine("ok", to: server)
        }

        let reply = try client.sendAndWaitReply("exec|payload")
        #expect(reply == "ok")
        #expect(received.lines.contains("event|pane.created|paneID=abc"))
    }

    @Test("EOF wakes a blocked sendAndWaitReply")
    func eofWakesBlockedCaller() {
        let (client, server) = makePair()
        client.startReading()

        Thread.detachNewThread { [server] in
            var buffer = [UInt8](repeating: 0, count: 256)
            _ = read(server, &buffer, buffer.count)
            close(server)
        }

        #expect(throws: HostSocketClient.ClientError.self) {
            _ = try client.sendAndWaitReply("identify|demo|token")
        }
    }
}

private final class EventBox: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: [String] = []

    func append(_ line: String) {
        lock.lock()
        defer { lock.unlock() }
        storage.append(line)
    }

    var lines: [String] {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }
}
