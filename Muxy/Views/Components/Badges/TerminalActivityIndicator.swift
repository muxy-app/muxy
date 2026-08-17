import SwiftUI

struct TerminalActivityIndicator: View {
    let activity: TerminalActivity

    var body: some View {
        Group {
            switch activity {
            case .working:
                ProgressView()
                    .controlSize(.mini)
                    .accessibilityLabel(L10n.string("Working"))
            case .waiting:
                Circle()
                    .fill(MuxyTheme.warning)
                    .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                    .accessibilityLabel(L10n.string("Waiting for attention"))
            case let .unread(count):
                NotificationBadge(count: count)
            case .finished:
                Circle()
                    .fill(MuxyTheme.accent)
                    .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                    .accessibilityLabel(L10n.string("Finished"))
            }
        }
        .help(Self.tooltip(for: activity))
    }

    static func tooltip(for activity: TerminalActivity) -> String {
        switch activity {
        case .working:
            return L10n.string("Work is in progress.")
        case .waiting:
            return L10n.string("An agent is waiting for your attention.")
        case let .unread(count):
            if count == 1 {
                return L10n.string("1 unread notification")
            }
            return L10n.string("\(count) unread notifications")
        case .finished:
            return L10n.string("Work finished and is ready to review.")
        }
    }
}
