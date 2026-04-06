import Foundation

@MainActor
@Observable
final class ToastState {
    static let shared = ToastState()

    var message: String?

    @ObservationIgnored private var dismissTask: Task<Void, Never>?

    private init() {}

    func show(_ message: String) {
        self.message = message
        dismissTask?.cancel()
        dismissTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            guard !Task.isCancelled, let self else { return }
            self.message = nil
        }
    }
}
