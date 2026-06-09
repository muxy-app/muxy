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

                SettingsToggleRow(label: "Show Status Bar", isOn: $showStatusBar)

                SettingsToggleRow(label: "Show Resource Usage in Status Bar", isOn: $showResourceUsage)
            }
        }
    }
}
