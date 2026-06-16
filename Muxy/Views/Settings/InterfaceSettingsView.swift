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

                TabHeaderWidthSettingRow()

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

private struct TabHeaderWidthSettingRow: View {
    @AppStorage(TabWidthPreferences.maxWidthKey) private var maxTabWidth = TabWidthPreferences.defaultMaxWidth

    private var sliderValue: Binding<Double> {
        Binding(
            get: { TabWidthPreferences.sliderValue(from: maxTabWidth) },
            set: { maxTabWidth = TabWidthPreferences.storedValue(forSlider: $0.rounded()) }
        )
    }

    private var valueLabel: String {
        TabWidthPreferences.effectiveMaxWidth(from: maxTabWidth)
            .map { "\(Int($0))px" } ?? "Full-width"
    }

    var body: some View {
        SettingsRow("Tab header width") {
            HStack(spacing: UIMetrics.spacing3) {
                Slider(
                    value: sliderValue,
                    in: TabWidthPreferences.minMaxWidth ... TabWidthPreferences.maxMaxWidth
                )
                Text(valueLabel)
                    .font(.system(size: SettingsMetrics.labelFontSize).monospacedDigit())
                    .foregroundStyle(SettingsStyle.mutedForeground)
                    .frame(width: 64, alignment: .trailing)
            }
            .frame(width: SettingsMetrics.controlWidth)
        }
    }
}
