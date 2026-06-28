import Foundation

struct ToastContent: Equatable {
    let title: String
    let body: String?
    let isActionable: Bool

    var accessibilityLabel: String {
        guard let body else { return title }
        return "\(title), \(body)"
    }
}

@MainActor
@Observable
final class ToastState {
    static let shared = ToastState()

    var content: ToastContent?

    var message: String? {
        content?.title
    }

    @ObservationIgnored private var dismissTask: Task<Void, Never>?
    @ObservationIgnored private var action: (@MainActor () -> Void)?

    private init() {}

    func show(_ message: String) {
        show(title: message)
    }

    func show(title: String, body: String? = nil, action: (@MainActor () -> Void)? = nil) {
        content = ToastContent(
            title: title,
            body: Self.normalizedBody(body),
            isActionable: action != nil
        )
        self.action = action
        dismissTask?.cancel()
        dismissTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 3_000_000_000)
            guard !Task.isCancelled, let self else { return }
            dismiss()
        }
    }

    func performAction() {
        let action = action
        dismiss()
        action?()
    }

    func dismiss() {
        content = nil
        action = nil
        dismissTask?.cancel()
        dismissTask = nil
    }

    private static func normalizedBody(_ body: String?) -> String? {
        guard let body else { return nil }
        let trimmed = body.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
