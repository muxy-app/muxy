import Foundation

enum AsyncTimeoutError: LocalizedError, Equatable {
    case timedOut(TimeInterval)

    var errorDescription: String? {
        switch self {
        case let .timedOut(seconds):
            "Operation timed out after \(Int(seconds))s"
        }
    }
}

enum AsyncTimeout {
    static func run<Value: Sendable>(
        seconds: TimeInterval,
        operation: @escaping @Sendable () async throws -> Value
    ) async throws -> Value {
        try await withThrowingTaskGroup(of: Value.self) { group in
            group.addTask { try await operation() }
            group.addTask {
                try await Task.sleep(for: .seconds(seconds))
                throw AsyncTimeoutError.timedOut(seconds)
            }
            defer { group.cancelAll() }
            guard let value = try await group.next() else {
                throw AsyncTimeoutError.timedOut(seconds)
            }
            return value
        }
    }
}

struct OperationDeadline: Sendable {
    private let timeout: TimeInterval
    private let expiresAt: TimeInterval

    init(timeout: TimeInterval) {
        self.timeout = timeout
        expiresAt = ProcessInfo.processInfo.systemUptime + timeout
    }

    func remaining() throws -> TimeInterval {
        let remaining = expiresAt - ProcessInfo.processInfo.systemUptime
        guard remaining > 0 else { throw AsyncTimeoutError.timedOut(timeout) }
        return remaining
    }
}
