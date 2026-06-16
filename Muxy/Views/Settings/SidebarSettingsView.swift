import SwiftUI

struct SidebarSettingsView: View {
    @State private var extensionStore = ExtensionStore.shared
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false
    @AppStorage(SidebarCollapsedStyle.storageKey) private var sidebarCollapsedStyle = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var sidebarExpandedStyle = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible
    @AppStorage(SidebarSelection.storageKey) private var activeSidebar = SidebarSelection.builtinValue
    @AppStorage(WorktreeListPreferences.showUnreadIndicatorKey)
    private var showWorktreeUnreadIndicator = WorktreeListPreferences.defaultShowUnreadIndicator
    @AppStorage(WorktreeListPreferences.orderByMRUKey)
    private var orderWorktreesByMRU = WorktreeListPreferences.defaultOrderByMRU

    private var providers: [ExtensionStore.ExtensionStatus] {
        SidebarSelection.availableProviders(store: extensionStore)
    }

    var body: some View {
        SettingsContainer {
            if !providers.isEmpty {
                SettingsSection("Active Sidebar") {
                    SettingsRow("Sidebar") {
                        HStack {
                            Spacer()
                            Picker("", selection: $activeSidebar) {
                                Text("Built-in").tag(SidebarSelection.builtinValue)
                                ForEach(providers) { status in
                                    Text(label(for: status)).tag(status.id)
                                }
                            }
                            .labelsHidden()
                            .fixedSize()
                        }
                        .frame(width: SettingsMetrics.controlWidth)
                    }
                }
            }

            SettingsSection("Layout") {
                SettingsToggleRow(label: "Show Home", isOn: $showHomeProject)

                SettingsToggleRow(
                    label: "Auto-expand worktrees on project switch",
                    isOn: $autoExpandWorktrees
                )

                SettingsRow("Collapsed Style") {
                    HStack {
                        Spacer()
                        Picker("", selection: $sidebarCollapsedStyle) {
                            ForEach(SidebarCollapsedStyle.allCases) { style in
                                Text(style.title).tag(style.rawValue)
                            }
                        }
                        .labelsHidden()
                        .pickerStyle(.segmented)
                        .fixedSize()
                    }
                    .frame(width: SettingsMetrics.controlWidth)
                }

                SettingsRow("Expanded Style") {
                    HStack {
                        Spacer()
                        Picker("", selection: $sidebarExpandedStyle) {
                            ForEach(SidebarExpandedStyle.allCases) { style in
                                Text(style.title).tag(style.rawValue)
                            }
                        }
                        .labelsHidden()
                        .pickerStyle(.segmented)
                        .fixedSize()
                    }
                    .frame(width: SettingsMetrics.controlWidth)
                }
            }

            SettingsSection("Worktrees", showsDivider: false) {
                SettingsToggleRow(
                    label: "Show unread notification indicator on worktrees",
                    isOn: $showWorktreeUnreadIndicator
                )

                SettingsToggleRow(
                    label: "Order worktrees by most-recently-used",
                    isOn: $orderWorktreesByMRU
                )
            }
        }
    }

    private func label(for status: ExtensionStore.ExtensionStatus) -> String {
        status.muxyExtension.manifest.sidebar?.title ?? status.muxyExtension.displayName
    }
}
