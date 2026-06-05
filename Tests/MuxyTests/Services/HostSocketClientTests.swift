import Darwin
import Foundation
import Testing

@testable import MuxyExtensionHost

@Suite("HostSocketClient connection retry")
struct HostSocketClientTests {
    @Test("connects to a listening server")
    func connectsToListeningServer() throws {
        let path = Self.temporarySocketPath()
        let listener = try Self.bindListener(at: path)
        defer {
            close(listener)
            unlink(path)
        }

        let client = try HostSocketClient(socketPath: path)
        #expect(client.isClosed == false)
    }

    @Test("gives up with connectFailed when no server appears")
    func givesUpWhenServerNeverAppears() {
        let path = Self.temporarySocketPath()

        var thrown: HostSocketClient.ClientError?
        do {
            _ = try HostSocketClient(socketPath: path, maxConnectAttempts: 3, connectRetryDelay: 0.01)
        } catch let error as HostSocketClient.ClientError {
            thrown = error
        } catch {}

        guard case .connectFailed = thrown else {
            Issue.record("expected connectFailed, got \(String(describing: thrown))")
            return
        }
    }

    private static func temporarySocketPath() -> String {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-host-test-\(UUID().uuidString).sock")
            .path
    }

    private static func bindListener(at path: String) throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            let bound = ptr.withMemoryRebound(to: CChar.self, capacity: 104) { $0 }
            _ = path.withCString { strncpy(bound, $0, 103) }
        }

        let result = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard result == 0, listen(descriptor, 5) == 0 else {
            close(descriptor)
            throw HostSocketClient.ClientError.connectFailed(String(cString: strerror(errno)))
        }
        return descriptor
    }
}
