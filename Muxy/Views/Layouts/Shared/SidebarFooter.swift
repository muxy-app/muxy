import SwiftUI

struct SidebarFooter: View {
    var isWide = true
    var sidebarExpanded = false

    @State private var showThemePicker = false
    @State private var showNotifications = false
    @State private var showTipsPopover = false
    @State private var showMuxy2BetaPromoPopover = false
    @State private var extensionStore = ExtensionStore.shared
    @State private var tipsStore = TipsStore.shared
    @AppStorage(TipsPreferences.visibleKey) private var showTips = TipsPreferences.defaultVisible
    @AppStorage(Muxy2BetaPromoPreferences.dismissedKey)
    private var muxy2BetaPromoDismissed = Muxy2BetaPromoPreferences.defaultDismissed

    private var notificationStore: NotificationStore { NotificationStore.shared }

    var body: some View {
        VStack(spacing: 0) {
            if isWide {
                if showsTip {
                    SidebarTipCard(store: tipsStore, onHide: hideTips)
                } else if showsMuxy2BetaPromo {
                    SidebarMuxy2BetaPromoCard(onDismiss: dismissMuxy2BetaPromo)
                }
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
        .onChange(of: showTips) { _, isVisible in
            guard !isVisible else { return }
            showTipsPopover = false
        }
    }

    private var showsTip: Bool {
        showTips && tipsStore.currentTip != nil
    }

    private var showsMuxy2BetaPromo: Bool {
        !showsTip && !muxy2BetaPromoDismissed
    }

    private func postToggleSidebar() {
        NotificationCenter.default.post(name: .toggleSidebar, object: nil)
    }

    private var sidebarToggleLabel: String {
        sidebarExpanded ? L10n.string("Collapse Sidebar") : L10n.string("Expand Sidebar")
    }

    private var notificationBellIcon: String {
        notificationStore.unreadCount > 0 ? "bell.badge" : "bell"
    }

    private func openExtensions() {
        NotificationCenter.default.post(name: .openExtensionsModal, object: nil)
    }

    private var extensionsHelp: String {
        guard extensionStore.hasUpdates else { return L10n.string("Extensions") }
        let count = extensionStore.updateCount
        return count == 1
            ? L10n.string("Extensions (1 update available)")
            : L10n.string("Extensions (\(count) updates available)")
    }

    private var extensionsAccessibilityLabel: String {
        extensionStore.hasUpdates
            ? L10n.string("Extensions, updates available")
            : L10n.string("Extensions")
    }

    private var collapsedFooter: some View {
        VStack(spacing: UIMetrics.spacing2) {
            if showsTip {
                tipsButton
            } else if showsMuxy2BetaPromo {
                muxy2BetaPromoButton
            }
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
            .help(L10n.string("\(sidebarToggleLabel) (\(KeyBindingStore.shared.combo(for: .toggleSidebar).displayString))"))
    }

    private var tipsButton: some View {
        IconButton(symbol: "lightbulb", accessibilityLabel: L10n.string("Show Muxy Tip")) {
            showTipsPopover.toggle()
        }
        .help(L10n.string("Show Muxy Tip"))
        .popover(isPresented: $showTipsPopover) {
            SidebarTipPopover(store: tipsStore, onHide: hideTips)
        }
    }

    private func hideTips() {
        showTipsPopover = false
        showTips = false
    }

    private var muxy2BetaPromoButton: some View {
        IconButton(symbol: "sparkles", accessibilityLabel: L10n.string("Try Muxy 2 Beta")) {
            showMuxy2BetaPromoPopover.toggle()
        }
        .help(L10n.string("Try Muxy 2 Beta"))
        .popover(isPresented: $showMuxy2BetaPromoPopover) {
            SidebarMuxy2BetaPromoPopover(onDismiss: dismissMuxy2BetaPromo)
        }
    }

    private func dismissMuxy2BetaPromo() {
        showMuxy2BetaPromoPopover = false
        muxy2BetaPromoDismissed = true
    }

    private var notificationsButton: some View {
        IconButton(symbol: notificationBellIcon, accessibilityLabel: L10n.string("Notifications")) { showNotifications.toggle() }
            .help(L10n.string("Notifications"))
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
        IconButton(symbol: "paintpalette", accessibilityLabel: L10n.string("Theme Picker")) { showThemePicker.toggle() }
            .help(L10n.string("Theme Picker (\(KeyBindingStore.shared.combo(for: .toggleThemePicker).displayString))"))
            .popover(isPresented: $showThemePicker) { ThemePicker(mode: .sidebar) }
    }
}
