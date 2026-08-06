import SwiftUI

struct TerminalActivityIndicator: View {
    let activity: TerminalActivity

    var body: some View {
        switch activity {
        case .working:
            ProgressView()
                .controlSize(.mini)
                .accessibilityLabel(L10n.string("Working"))
        case .waiting:
            Image(systemName: "questionmark.circle.fill")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.warning)
                .accessibilityLabel(L10n.string("Waiting for attention"))
        case let .unread(count):
            NotificationBadge(count: count)
        case .finished:
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
                .accessibilityLabel(L10n.string("Finished"))
        }
    }
}
