import SwiftUI

struct InterfaceSettingsView: View {
    @State private var uiScale = UIScale.shared
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false
    @AppStorage(SidebarCollapsedStyle.storageKey) private var sidebarCollapsedStyle = SidebarCollapsedStyle.defaultValue.rawValue
    @AppStorage(SidebarExpandedStyle.storageKey) private var sidebarExpandedStyle = SidebarExpandedStyle.defaultValue.rawValue
    @AppStorage("muxy.showStatusBar") private var showStatusBar = true

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

                TabHeaderSizeSettingRow()

                SettingsToggleRow(label: "Show Status Bar", isOn: $showStatusBar)
            }

            SettingsSection("Sidebar", showsDivider: false) {
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

private struct TabHeaderSizeSettingRow: View {
    @State private var selectedSize = TabWidthPreferences.currentHeaderSize()

    var body: some View {
        SettingsRow("Tab header size") {
            Picker("", selection: $selectedSize) {
                ForEach(TabWidthPreferences.HeaderSize.allCases) { size in
                    Text(size.title).tag(size)
                }
            }
            .labelsHidden()
            .pickerStyle(.segmented)
            .frame(width: SettingsMetrics.controlWidth)
        }
        .onAppear {
            selectedSize = TabWidthPreferences.currentHeaderSize()
        }
        .onChange(of: selectedSize) { _, newValue in
            TabWidthPreferences.store(newValue)
        }
        .onReceive(NotificationCenter.default.publisher(for: UserDefaults.didChangeNotification)) { _ in
            let currentSize = TabWidthPreferences.currentHeaderSize()
            if currentSize != selectedSize {
                selectedSize = currentSize
            }
        }
    }
}
