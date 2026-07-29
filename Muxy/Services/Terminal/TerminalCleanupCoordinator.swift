import Foundation

@MainActor
final class TerminalCleanupCoordinator {
    typealias Cleanup = @MainActor () async -> Void

    static let shared = TerminalCleanupCoordinator()

    private var tasks: [UUID: Task<Void, Never>] = [:]

    var outstandingCount: Int {
        tasks.count
    }

    @discardableResult
    func schedule(
        after precedingTasks: [Task<Void, Never>] = [],
        cleanup: @escaping Cleanup
    ) -> Task<Void, Never> {
        let taskID = UUID()
        let task = Task { @MainActor [weak self] in
            for precedingTask in precedingTasks {
                await precedingTask.value
            }
            guard !Task.isCancelled else {
                self?.tasks.removeValue(forKey: taskID)
                return
            }
            await cleanup()
            self?.tasks.removeValue(forKey: taskID)
        }
        tasks[taskID] = task
        return task
    }

    func waitUntilIdle() async {
        await withTaskCancellationHandler {
            while !tasks.isEmpty, !Task.isCancelled {
                let outstandingTasks = Array(tasks.values)
                for task in outstandingTasks {
                    await task.value
                    guard !Task.isCancelled else { return }
                }
            }
        } onCancel: {
            Task { @MainActor [weak self] in
                self?.cancelAll()
            }
        }
    }

    func cancelAll() {
        let outstandingTasks = Array(tasks.values)
        tasks.removeAll(keepingCapacity: false)
        for task in outstandingTasks {
            task.cancel()
        }
    }
}
