import SwiftUI

struct NotificationPanel: View {
    @Environment(AppState.self) private var appState
    let onDismiss: () -> Void

    private var store: NotificationStore { NotificationStore.shared }

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            if store.notifications.isEmpty {
                emptyState
            } else {
                notificationList
            }
        }
        .frame(width: 320, height: 400)
        .background(MuxyTheme.bg)
    }

    private var header: some View {
        HStack {
            Text("Notifications")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Spacer()
            if !store.notifications.isEmpty {
                Button("Clear") {
                    store.clear()
                }
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Spacer()
            Image(systemName: "bell.slash")
                .font(.system(size: 24, weight: .light))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("No notifications")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
            Spacer()
        }
        .frame(maxWidth: .infinity)
    }

    private var notificationList: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(store.notifications) { notification in
                    NotificationRow(notification: notification) {
                        NotificationNavigator.navigate(
                            to: notification,
                            appState: appState,
                            notificationStore: store
                        )
                        onDismiss()
                    }
                    Rectangle().fill(MuxyTheme.border).frame(height: 1)
                }
            }
        }
    }
}

private struct NotificationRow: View {
    let notification: MuxyNotification
    let onTap: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: onTap) {
            HStack(alignment: .top, spacing: 8) {
                Circle()
                    .fill(notification.isRead ? Color.clear : MuxyTheme.accent)
                    .frame(width: 6, height: 6)
                    .padding(.top, 5)

                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Image(systemName: sourceIcon)
                            .font(.system(size: 10))
                            .foregroundStyle(MuxyTheme.fgMuted)
                        Text(notification.title)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(MuxyTheme.fg)
                            .lineLimit(1)
                        Spacer()
                        Text(relativeTimestamp)
                            .font(.system(size: 10))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }

                    if !notification.body.isEmpty {
                        Text(notification.body)
                            .font(.system(size: 11))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .lineLimit(2)
                    }
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .contentShape(Rectangle())
            .background(hovered ? MuxyTheme.hover : Color.clear)
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }

    private var sourceIcon: String {
        switch notification.source {
        case .osc: "terminal"
        case .claudeHook: "sparkles"
        case .socket: "network"
        case .vcs: "arrow.triangle.branch"
        }
    }

    private var relativeTimestamp: String {
        let interval = Date().timeIntervalSince(notification.timestamp)
        guard interval >= 60 else { return "now" }
        let minutes = Int(interval / 60)
        guard minutes >= 60 else { return "\(minutes)m" }
        let hours = minutes / 60
        guard hours >= 24 else { return "\(hours)h" }
        return "\(hours / 24)d"
    }
}
