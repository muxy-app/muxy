import SwiftUI

struct GeneralSettingsView: View {
    @AppStorage(MuxySettings.quickTerminalWidthFractionKey)
    private var quickTerminalWidth = MuxySettings.defaultQuickTerminalWidthFraction

    @AppStorage(MuxySettings.quickTerminalHeightFractionKey)
    private var quickTerminalHeight = MuxySettings.defaultQuickTerminalHeightFraction

    @AppStorage(MuxySettings.hideTabBarWhenSingleTabKey)
    private var hideTabBarWhenSingleTab = MuxySettings.defaultHideTabBarWhenSingleTab

    @AppStorage(MuxySettings.windowBackgroundOpacityKey)
    private var windowOpacity = MuxySettings.defaultWindowBackgroundOpacity

    @AppStorage(MuxySettings.windowBackgroundBlurKey)
    private var windowBlur = MuxySettings.defaultWindowBackgroundBlur

    var body: some View {
        Form {
            Section("Quick Terminal") {
                LabeledContent("Width") {
                    HStack {
                        Slider(value: $quickTerminalWidth, in: 0.2 ... 1.0, step: 0.05)
                        Text("\(Int(quickTerminalWidth * 100))%")
                            .monospacedDigit()
                            .frame(width: 36, alignment: .trailing)
                    }
                }

                LabeledContent("Height") {
                    HStack {
                        Slider(value: $quickTerminalHeight, in: 0.2 ... 1.0, step: 0.05)
                        Text("\(Int(quickTerminalHeight * 100))%")
                            .monospacedDigit()
                            .frame(width: 36, alignment: .trailing)
                    }
                }
            }

            Section("Tabs") {
                Toggle("Hide tab bar when there is only one tab", isOn: $hideTabBarWhenSingleTab)
            }

            Section("Window") {
                LabeledContent("Background Opacity") {
                    HStack {
                        Slider(value: $windowOpacity, in: 0.1 ... 1.0, step: 0.05)
                            .disabled(windowBlur)
                        Text("\(Int(windowOpacity * 100))%")
                            .monospacedDigit()
                            .frame(width: 36, alignment: .trailing)
                            .foregroundStyle(windowBlur ? .secondary : .primary)
                    }
                }

                Toggle("Background Blur (Liquid Glass)", isOn: $windowBlur)
            }
        }
        .formStyle(.grouped)
        .onChange(of: windowOpacity) { _, _ in notifyWindowUpdate() }
        .onChange(of: windowBlur) { _, _ in notifyWindowUpdate() }
    }

    private func notifyWindowUpdate() {
        NotificationCenter.default.post(name: .windowBackgroundSettingChanged, object: nil)
    }
}
