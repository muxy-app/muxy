import Foundation

@MainActor
final class TerminalTerminationCleanup {
    typealias Cleanup = @MainActor () async -> Void

    private(set) var isRunning = false
    private(set) var isComplete = false
    private var cleanupTask: Task<Void, Never>?
    private var timeoutTask: Task<Void, Never>?

    func start(
        timeout: Duration,
        cleanup: @escaping Cleanup,
        completion: @escaping @MainActor () -> Void
    ) {
        guard !isRunning, !isComplete else { return }
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
            finish(completion: completion)
        }
        Task { @MainActor [weak self] in
            await cleanupTask.value
            self?.finish(completion: completion)
        }
    }

    private func finish(completion: @escaping @MainActor () -> Void) {
        guard isRunning else { return }
        isRunning = false
        isComplete = true
        timeoutTask?.cancel()
        timeoutTask = nil
        cleanupTask = nil
        completion()
    }
}
