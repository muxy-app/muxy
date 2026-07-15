import Foundation

@MainActor
protocol RemoteMacSocket: AnyObject {
    func connect(to url: URL)
    func send(_ data: Data) async throws
    func receive() async throws -> Data
    func disconnect()
}

@MainActor
final class URLSessionRemoteMacSocket: RemoteMacSocket {
    private var session: URLSession?
    private var task: URLSessionWebSocketTask?

    func connect(to url: URL) {
        disconnect()
        let session = URLSession(configuration: .default)
        let task = session.webSocketTask(with: url)
        self.session = session
        self.task = task
        task.resume()
    }

    func send(_ data: Data) async throws {
        guard let task else { throw RemoteMacConnectionError.disconnected }
        try await task.send(.data(data))
    }

    func receive() async throws -> Data {
        guard let task else { throw RemoteMacConnectionError.disconnected }
        switch try await task.receive() {
        case let .data(data):
            return data
        case let .string(text):
            return Data(text.utf8)
        @unknown default:
            throw RemoteMacConnectionError.invalidMessage
        }
    }

    func disconnect() {
        task?.cancel(with: .goingAway, reason: nil)
        task = nil
        session?.invalidateAndCancel()
        session = nil
    }
}
