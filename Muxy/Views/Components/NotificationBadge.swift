import SwiftUI

struct NotificationBadge: View {
    let count: Int

    var body: some View {
        Circle()
            .fill(MuxyTheme.accent)
            .frame(width: UIMetrics.scaled(8), height: UIMetrics.scaled(8))
            .accessibilityLabel("\(count) unread notification\(count == 1 ? "" : "s")")
    }
}
