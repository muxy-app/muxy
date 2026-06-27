import SwiftUI

struct SidebarFooter: View {
    var isWide = true
    var sidebarExpanded = false

    @State private var showThemePicker = false
    @State private var showNotifications = false
    @State private var extensionStore = ExtensionStore.shared

    private var notificationStore: NotificationStore { NotificationStore.shared }

    var body: some View {
        VStack(spacing: 0) {
            if isWide {
                expandedFooter
            } else {
                collapsedFooter
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleThemePicker)) { _ in
            showThemePicker.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .toggleNotificationPanel)) { _ in
            showNotifications.toggle()
        }
    }

    private func postToggleSidebar() {
        NotificationCenter.default.post(name: .toggleSidebar, object: nil)
    }

    private var sidebarToggleLabel: String {
        sidebarExpanded ? "Collapse Sidebar" : "Expand Sidebar"
    }

    private var notificationBellIcon: String {
        notificationStore.unreadCount > 0 ? "bell.badge" : "bell"
    }

    private func openExtensions() {
        NotificationCenter.default.post(name: .openExtensionsModal, object: nil)
    }

    private var extensionsHelp: String {
        guard extensionStore.hasUpdates else { return "Extensions" }
        let count = extensionStore.updateCount
        return count == 1 ? "Extensions (1 update available)" : "Extensions (\(count) updates available)"
    }

    private var extensionsAccessibilityLabel: String {
        extensionStore.hasUpdates ? "Extensions, updates available" : "Extensions"
    }

    private var collapsedFooter: some View {
        VStack(spacing: UIMetrics.spacing2) {
            notificationsButton
            extensionsButton
            themeButton
            sidebarToggleButton
        }
        .padding(.bottom, UIMetrics.spacing4)
    }

    private var expandedFooter: some View {
        HStack(spacing: UIMetrics.spacing2) {
            sidebarToggleButton
            Spacer()
            notificationsButton
            extensionsButton
            themeButton
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.bottom, UIMetrics.spacing4)
    }

    private var sidebarToggleButton: some View {
        IconButton(symbol: "sidebar.left", accessibilityLabel: sidebarToggleLabel) { postToggleSidebar() }
            .help("\(sidebarToggleLabel) (\(KeyBindingStore.shared.combo(for: .toggleSidebar).displayString))")
    }

    private var notificationsButton: some View {
        IconButton(symbol: notificationBellIcon, accessibilityLabel: "Notifications") { showNotifications.toggle() }
            .help("Notifications")
            .popover(isPresented: $showNotifications) {
                NotificationPanel(onDismiss: { showNotifications = false })
            }
    }

    private var extensionsButton: some View {
        IconButton(
            symbol: "puzzlepiece.extension",
            showsBadge: extensionStore.hasUpdates,
            accessibilityLabel: extensionsAccessibilityLabel
        ) { openExtensions() }
            .help(extensionsHelp)
    }

    private var themeButton: some View {
        IconButton(symbol: "paintpalette", accessibilityLabel: "Theme Picker") { showThemePicker.toggle() }
            .help("Theme Picker (\(KeyBindingStore.shared.combo(for: .toggleThemePicker).displayString))")
            .popover(isPresented: $showThemePicker) { ThemePicker(mode: .sidebar) }
    }
}
