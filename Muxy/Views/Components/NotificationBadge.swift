import SwiftUI

struct NotificationBadge: View {
    let count: Int

    var body: some View {
        Circle()
            .fill(MuxyTheme.accent)
            .frame(width: UIMetrics.spacing4, height: UIMetrics.spacing4)
            .accessibilityLabel("\(count) unread notification\(count == 1 ? "" : "s")")
    }
}
