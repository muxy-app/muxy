import Foundation

@MainActor
final class TerminalInputTransactionHandle {
    private var result: Bool?
    private var continuations: [CheckedContinuation<Bool, Never>] = []

    func value() async -> Bool {
        if let result {
            return result
        }
        return await withCheckedContinuation { continuation in
            continuations.append(continuation)
        }
    }

    fileprivate func finish(with result: Bool) {
        guard self.result == nil else { return }
        self.result = result
        let continuations = continuations
        self.continuations.removeAll(keepingCapacity: false)
        for continuation in continuations {
            continuation.resume(returning: result)
        }
    }
}

@MainActor
final class TerminalInputQueue {
    typealias Operation = @MainActor () async -> Bool

    @TaskLocal private static var activeQueueID: ObjectIdentifier?

    private struct PendingOperation {
        let operation: Operation
        let handle: TerminalInputTransactionHandle
    }

    private var pendingOperations: [PendingOperation] = []
    private var pendingIndex = 0
    private var worker: Task<Void, Never>?
    private var generation = 0

    var hasPendingOperations: Bool {
        worker != nil || pendingIndex < pendingOperations.count
    }

    func enqueue(_ operation: @escaping @MainActor () async -> Void) {
        enqueueTransaction {
            await operation()
            return !Task.isCancelled
        }
    }

    @discardableResult
    func enqueueTransaction(_ operation: @escaping Operation) -> TerminalInputTransactionHandle {
        let handle = TerminalInputTransactionHandle()
        append(operation: operation, handle: handle)
        return handle
    }

    func perform(_ operation: @escaping Operation) async -> Bool {
        if Self.activeQueueID == ObjectIdentifier(self) {
            return await operation()
        }
        return await enqueueTransaction(operation).value()
    }

    @discardableResult
    func deferIfPending(_ operation: @escaping @MainActor () async -> Void) -> Bool {
        guard hasPendingOperations, Self.activeQueueID != ObjectIdentifier(self) else { return false }
        enqueue(operation)
        return true
    }

    @discardableResult
    func cancelAll() -> Task<Void, Never>? {
        generation += 1
        let cancelledWorker = worker
        worker?.cancel()
        finishPendingOperations(result: false)
        return cancelledWorker
    }

    func waitUntilIdle() async {
        while let worker {
            await worker.value
        }
    }

    private func append(
        operation: @escaping Operation,
        handle: TerminalInputTransactionHandle
    ) {
        pendingOperations.append(PendingOperation(operation: operation, handle: handle))
        startWorkerIfNeeded()
    }

    private func startWorkerIfNeeded() {
        guard worker == nil, pendingIndex < pendingOperations.count else { return }
        let workerGeneration = generation
        worker = Task { @MainActor [weak self] in
            guard let self else { return }
            await Self.$activeQueueID.withValue(ObjectIdentifier(self)) {
                await run(workerGeneration: workerGeneration)
            }
            finishWorker(workerGeneration: workerGeneration)
        }
    }

    private func run(workerGeneration: Int) async {
        while generation == workerGeneration, !Task.isCancelled, let pending = nextOperation() {
            let result = await pending.operation()
            pending.handle.finish(with: result && !Task.isCancelled && generation == workerGeneration)
        }
    }

    private func nextOperation() -> PendingOperation? {
        guard pendingIndex < pendingOperations.count else { return nil }
        let operation = pendingOperations[pendingIndex]
        pendingIndex += 1
        return operation
    }

    private func finishWorker(workerGeneration: Int) {
        guard generation == workerGeneration else {
            worker = nil
            startWorkerIfNeeded()
            return
        }
        compactPendingOperations()
        worker = nil
        startWorkerIfNeeded()
    }

    private func finishPendingOperations(result: Bool) {
        for index in pendingIndex ..< pendingOperations.count {
            pendingOperations[index].handle.finish(with: result)
        }
        pendingOperations.removeAll(keepingCapacity: false)
        pendingIndex = 0
    }

    private func compactPendingOperations() {
        guard pendingIndex > 0 else { return }
        pendingOperations.removeFirst(pendingIndex)
        pendingIndex = 0
    }
}
