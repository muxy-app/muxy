import SwiftUI

struct NotificationBadge: View {
    let count: Int

    var body: some View {
        Circle()
            .fill(MuxyTheme.accent)
            .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
            .accessibilityLabel(
                count == 1
                    ? L10n.string("1 unread notification")
                    : L10n.string("\(count) unread notifications")
            )
    }
}
