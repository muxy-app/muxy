import Foundation

@MainActor
final class ExtensionLogCoalescer {
    static let flushInterval: TimeInterval = 0.05

    private let onUpdate: @MainActor (ExtensionLogUpdate) -> Void
    private var pendingAppend: [String] = []
    private var flushWork: DispatchWorkItem?

    init(onUpdate: @escaping @MainActor (ExtensionLogUpdate) -> Void) {
        self.onUpdate = onUpdate
    }

    func ingest(_ update: ExtensionLogUpdate) {
        switch update {
        case let .reset(lines):
            cancelPending()
            onUpdate(.reset(lines))
        case let .append(lines):
            scheduleAppend(lines)
        }
    }

    func cancel() {
        cancelPending()
    }

    private func scheduleAppend(_ lines: [String]) {
        guard !lines.isEmpty else { return }
        pendingAppend.append(contentsOf: lines)
        guard flushWork == nil else { return }
        let work = DispatchWorkItem { [weak self] in
            self?.flush()
        }
        flushWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.flushInterval, execute: work)
    }

    private func flush() {
        flushWork = nil
        guard !pendingAppend.isEmpty else { return }
        let lines = pendingAppend
        pendingAppend = []
        onUpdate(.append(lines))
    }

    private func cancelPending() {
        flushWork?.cancel()
        flushWork = nil
        pendingAppend = []
    }
}
