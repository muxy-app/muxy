import SwiftUI

struct InterfaceSettingsView: View {
    @State private var uiScale = UIScale.shared
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false
    @AppStorage(SidebarCollapsedStyle.storageKey) private var sidebarCollapsedStyle = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var sidebarExpandedStyle = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage("muxy.showStatusBar") private var showStatusBar = true
    @AppStorage(ResourceUsagePreferences.visibleKey) private var showResourceUsage = ResourceUsagePreferences.defaultVisible
    @AppStorage(HomeProjectPreferences.visibleKey) private var showHomeProject = HomeProjectPreferences.defaultVisible

    var body: some View {
        SettingsContainer {
            SettingsSection("Interface") {
                SettingsRow("Size") {
                    Picker("", selection: $uiScale.preset) {
                        ForEach(UIScale.Preset.allCases) { preset in
                            Text(preset.title).tag(preset)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .frame(width: SettingsMetrics.controlWidth)
                }

                SettingsToggleRow(label: "Show Status Bar", isOn: $showStatusBar)

                SettingsToggleRow(label: "Show Resource Usage in Status Bar", isOn: $showResourceUsage)
            }

            SettingsSection("Sidebar", showsDivider: false) {
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
        }
    }
}
