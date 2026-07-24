import Foundation

protocol DebounceScheduler: Sendable {
    func schedule(after interval: TimeInterval, _ work: @escaping @Sendable () -> Void) -> DebounceCancellable
}

protocol DebounceCancellable {
    func cancel()
}

struct QueueDebounceScheduler: DebounceScheduler {
    let queue: DispatchQueue

    func schedule(after interval: TimeInterval, _ work: @escaping @Sendable () -> Void) -> DebounceCancellable {
        let item = DispatchWorkItem(block: work)
        queue.asyncAfter(deadline: .now() + interval, execute: item)
        return WorkItemCancellable(item: item)
    }

    private struct WorkItemCancellable: DebounceCancellable {
        let item: DispatchWorkItem
        func cancel() {
            item.cancel()
        }
    }
}

final class DebouncedTrigger: @unchecked Sendable {
    private let lock = NSLock()
    private let interval: TimeInterval
    private let scheduler: DebounceScheduler
    private let handler: @Sendable () -> Void
    private var pending: DebounceCancellable?

    init(
        interval: TimeInterval,
        scheduler: DebounceScheduler,
        handler: @escaping @Sendable () -> Void
    ) {
        self.interval = interval
        self.scheduler = scheduler
        self.handler = handler
    }

    func signal() {
        lock.lock()
        pending?.cancel()
        let handler = handler
        pending = scheduler.schedule(after: interval) { [weak self] in
            self?.clearPending()
            handler()
        }
        lock.unlock()
    }

    private func clearPending() {
        lock.lock()
        pending = nil
        lock.unlock()
    }
}
