import SwiftUI

struct InterfaceSettingsView: View {
    @State private var uiScale = UIScale.shared
    @AppStorage("muxy.showStatusBar") private var showStatusBar = true
    @AppStorage(ResourceUsagePreferences.visibleKey) private var showResourceUsage = ResourceUsagePreferences.defaultVisible

    var body: some View {
        SettingsContainer {
            SettingsSection("Interface", showsDivider: false) {
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

                SettingsToggleRow(label: "Show Resource Usage in Status Bar", isOn: $showResourceUsage)
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
