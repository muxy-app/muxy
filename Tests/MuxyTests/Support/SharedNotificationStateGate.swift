import Foundation

@MainActor
enum SharedNotificationStateGate {
    private static var isBusy = false
    private static var waiters: [CheckedContinuation<Void, Never>] = []

    static func run<T>(_ body: () async throws -> T) async rethrows -> T {
        await acquire()
        defer { release() }
        return try await body()
    }

    private static func acquire() async {
        while isBusy {
            await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
                waiters.append(continuation)
            }
        }
        isBusy = true
    }

    private static func release() {
        isBusy = false
        let pending = waiters
        waiters.removeAll()
        for waiter in pending {
            waiter.resume()
        }
    }
}
