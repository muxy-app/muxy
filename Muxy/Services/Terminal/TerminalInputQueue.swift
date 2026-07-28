import Foundation

@MainActor
final class TerminalInputQueue {
    typealias Operation = @MainActor () async -> Void

    @TaskLocal private static var activeQueueID: ObjectIdentifier?

    private var tail: Task<Void, Never>?
    private var generation = 0
    private var sequence = 0
    private var pendingCount = 0

    var hasPendingOperations: Bool {
        pendingCount > 0
    }

    func enqueue(_ operation: @escaping Operation) {
        let precedingTask = tail
        let operationGeneration = generation
        sequence += 1
        let operationSequence = sequence
        pendingCount += 1

        tail = Task { @MainActor [weak self] in
            await precedingTask?.value
            guard let self, generation == operationGeneration, !Task.isCancelled else { return }
            await Self.$activeQueueID.withValue(ObjectIdentifier(self)) {
                await operation()
            }
            complete(sequence: operationSequence, generation: operationGeneration)
        }
    }

    @discardableResult
    func deferIfPending(_ operation: @escaping Operation) -> Bool {
        guard hasPendingOperations, Self.activeQueueID != ObjectIdentifier(self) else { return false }
        enqueue(operation)
        return true
    }

    func cancelAll() {
        generation += 1
        tail?.cancel()
        tail = nil
        pendingCount = 0
    }

    func waitUntilIdle() async {
        while let tail {
            await tail.value
        }
    }

    private func complete(sequence operationSequence: Int, generation operationGeneration: Int) {
        guard generation == operationGeneration else { return }
        pendingCount -= 1
        if sequence == operationSequence {
            tail = nil
        }
    }
}
