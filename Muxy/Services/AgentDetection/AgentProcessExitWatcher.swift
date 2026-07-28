import Foundation

@MainActor
final class AgentProcessExitWatcher {
    var onExit: (() -> Void)?

    private var source: DispatchSourceProcess?
    private var watchedProcessID: Int32?

    var processID: Int32? { watchedProcessID }

    func watch(processID: Int32) {
        guard processID > 0, watchedProcessID != processID else { return }
        cancel()
        let source = DispatchSource.makeProcessSource(identifier: processID, eventMask: .exit, queue: .main)
        source.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                self?.handleExit()
            }
        }
        watchedProcessID = processID
        self.source = source
        source.resume()
    }

    func cancel() {
        source?.cancel()
        source = nil
        watchedProcessID = nil
    }

    private func handleExit() {
        cancel()
        onExit?()
    }
}
