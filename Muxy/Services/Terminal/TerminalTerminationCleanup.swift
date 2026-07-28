import Foundation

@MainActor
final class TerminalTerminationCleanup {
    typealias Cleanup = @MainActor () async -> Void

    private(set) var isRunning = false
    private(set) var isComplete = false
    private var cleanupTask: Task<Void, Never>?
    private var timeoutTask: Task<Void, Never>?
    private var completions: [@MainActor () -> Void] = []

    func start(
        timeout: Duration,
        cleanup: @escaping Cleanup,
        completion: @escaping @MainActor () -> Void
    ) {
        guard !isComplete else {
            completion()
            return
        }
        completions.append(completion)
        guard !isRunning else { return }
        isRunning = true
        let cleanupTask = Task { @MainActor in
            await cleanup()
        }
        self.cleanupTask = cleanupTask
        timeoutTask = Task { @MainActor [weak self] in
            do {
                try await Task.sleep(for: timeout)
            } catch {
                return
            }
            guard let self else { return }
            cleanupTask.cancel()
            finish()
        }
        Task { @MainActor [weak self] in
            await cleanupTask.value
            self?.finish()
        }
    }

    private func finish() {
        guard isRunning else { return }
        isRunning = false
        isComplete = true
        timeoutTask?.cancel()
        timeoutTask = nil
        cleanupTask = nil
        let pendingCompletions = completions
        completions.removeAll(keepingCapacity: false)
        for completion in pendingCompletions {
            completion()
        }
    }
}
