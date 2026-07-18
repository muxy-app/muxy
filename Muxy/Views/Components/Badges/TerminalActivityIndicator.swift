import SwiftUI

struct TerminalActivityIndicator: View {
    let activity: TerminalActivity

    var body: some View {
        switch activity {
        case .working:
            ProgressView()
                .controlSize(.mini)
                .accessibilityLabel("Working")
        case .waiting:
            Circle()
                .fill(MuxyTheme.warning)
                .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                .accessibilityLabel("Waiting for attention")
        case let .unread(count):
            NotificationBadge(count: count)
        case .finished:
            Circle()
                .fill(MuxyTheme.accent)
                .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
                .accessibilityLabel("Finished")
        }
    }
}
