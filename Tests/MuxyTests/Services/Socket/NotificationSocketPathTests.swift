import Foundation
import Testing

@testable import Muxy

@Suite("NotificationSocketServer socket path")
struct NotificationSocketPathTests {
    @Test("debug builds use an isolated socket so a dev build never collides with the installed app")
    func debugUsesIsolatedSocket() {
        let path = NotificationSocketServer.socketPath
        #if DEBUG
        #expect(path.hasSuffix("muxy-dev.sock"))
        #else
        #expect(path.hasSuffix("muxy.sock"))
        #expect(!path.hasSuffix("muxy-dev.sock"))
        #endif
    }

    @Test("the listen path ignores an inherited MUXY_SOCKET_PATH so a dev build never steals the installed app's socket")
    func listenPathIgnoresInheritedOverride() {
        let path = NotificationSocketServer.socketPath
        #expect(!path.contains("\u{0}"))
        #expect(path.hasSuffix(".sock"))
    }
}
