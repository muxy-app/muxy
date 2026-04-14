import SwiftUI

struct NotificationPanelItem: Identifiable {
    let id: UUID
    let sourceIcon: String
    let title: String
    let body: String
    let timestamp: Date
    let isRead: Bool

    var relativeTimestamp: String {
        let interval = Date().timeIntervalSince(timestamp)
        guard interval >= 60 else { return "now" }
        let minutes = Int(interval / 60)
        guard minutes >= 60 else { return "\(minutes)m" }
        let hours = minutes / 60
        guard hours >= 24 else { return "\(hours)h" }
        return "\(hours / 24)d"
    }
}

struct NotificationPanel: View {
    @Environment(AppState.self) private var appState
    let onDismiss: () -> Void

    private var items: [NotificationPanelItem] {
        let store = NotificationStore.shared
        _ = store.readStateVersion
        let registry = AIProviderRegistry.shared
        return store.notifications.map { n in
            NotificationPanelItem(
                id: n.id,
                sourceIcon: registry.iconName(for: n.source),
                title: n.title,
                body: n.body,
                timestamp: n.timestamp,
                isRead: n.isRead
            )
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            let currentItems = items
            if currentItems.isEmpty {
                emptyState
            } else {
                header
                Divider().overlay(MuxyTheme.border)
                notificationList(currentItems)
            }
        }
        .frame(width: 320, height: 400)
    }

    private var header: some View {
        HStack {
            Text("Notifications")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Spacer()
            Button {
                NotificationStore.shared.clear()
            } label: {
                Text("Clear All")
                    .font(.system(size: 11))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
    }

    private func notificationList(_ currentItems: [NotificationPanelItem]) -> some View {
        ScrollView(.vertical, showsIndicators: false) {
            LazyVStack(spacing: 0) {
                ForEach(currentItems) { item in
                    NotificationRow(item: item, isHighlighted: false, onRemove: {
                        NotificationStore.shared.remove(item.id)
                    })
                    .contentShape(Rectangle())
                    .onTapGesture { selectItem(item) }
                }
            }
            .padding(.vertical, 4)
        }
        .background(MuxyTheme.bg)
    }

    private var emptyState: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Notifications")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer()
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)

            Divider().overlay(MuxyTheme.border)

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
        .background(MuxyTheme.bg)
    }

    private func selectItem(_ item: NotificationPanelItem) {
        let store = NotificationStore.shared
        guard let notification = store.notifications.first(where: { $0.id == item.id }) else { return }
        NotificationNavigator.navigate(
            to: notification,
            appState: appState,
            notificationStore: store
        )
        onDismiss()
    }
}

private struct NotificationRow: View {
    let item: NotificationPanelItem
    let isHighlighted: Bool
    let onRemove: () -> Void
    @State private var hovered = false

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Circle()
                .fill(item.isRead ? Color.clear : MuxyTheme.accent)
                .frame(width: 6, height: 6)
                .padding(.top, 5)

            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Image(systemName: item.sourceIcon)
                        .font(.system(size: 10))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    Text(item.title)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                    Spacer()
                    Text(item.relativeTimestamp)
                        .font(.system(size: 10))
                        .foregroundStyle(MuxyTheme.fgMuted)
                    if hovered {
                        dismissButton
                    }
                }

                if !item.body.isEmpty {
                    Text(item.body)
                        .font(.system(size: 11))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .lineLimit(2)
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .background(isHighlighted ? MuxyTheme.surface : (hovered ? MuxyTheme.hover : .clear))
        .onHover { hovered = $0 }
    }

    private var dismissButton: some View {
        Button {
            onRemove()
        } label: {
            Image(systemName: "xmark")
                .font(.system(size: 8, weight: .bold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 14, height: 14)
                .background(MuxyTheme.surface, in: Circle())
        }
        .buttonStyle(.plain)
    }
}
