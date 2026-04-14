import SwiftUI

struct NotificationPanelItem: Identifiable {
    let id: UUID
    let notificationID: UUID
    let sourceIcon: String
    let title: String
    let body: String
    let timestamp: Date
    let isRead: Bool

    var searchText: String { "\(title) \(body)" }

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

    @State private var items: [NotificationPanelItem] = []

    var body: some View {
        VStack(spacing: 0) {
            if items.isEmpty {
                emptySearchableList
            } else {
                SearchableListPicker(
                    items: items,
                    filterKey: \.searchText,
                    placeholder: "Search notifications",
                    emptyLabel: "No matching notifications",
                    onSelect: { selectItem($0) },
                    row: { item, isHighlighted in
                        NotificationRow(item: item, isHighlighted: isHighlighted)
                    }
                )
            }
        }
        .frame(width: 320, height: 400)
        .onAppear { loadItems() }
    }

    private var emptySearchableList: some View {
        VStack(spacing: 0) {
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .font(.system(size: 12))
                Text("Search notifications")
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgDim)
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

    private func loadItems() {
        let registry = AIProviderRegistry.shared
        items = NotificationStore.shared.notifications.map { n in
            NotificationPanelItem(
                id: n.id,
                notificationID: n.id,
                sourceIcon: registry.iconName(for: n.source),
                title: n.title,
                body: n.body,
                timestamp: n.timestamp,
                isRead: n.isRead
            )
        }
    }

    private func selectItem(_ item: NotificationPanelItem) {
        let store = NotificationStore.shared
        guard let notification = store.notifications.first(where: { $0.id == item.notificationID }) else { return }
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
}
