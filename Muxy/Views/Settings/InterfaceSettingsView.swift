import SwiftUI

struct InterfaceSettingsView: View {
    @State private var uiScale = UIScale.shared
    @State private var themeService = ThemeService.shared
    @State private var showLightThemePicker = false
    @State private var showDarkThemePicker = false
    @State private var currentLightTheme: String?
    @State private var currentDarkTheme: String?
    @AppStorage("muxy.showStatusBar") private var showStatusBar = true
    @AppStorage(ResourceUsagePreferences.visibleKey) private var showResourceUsage = ResourceUsagePreferences.defaultVisible

    var body: some View {
        SettingsContainer {
            SettingsSection("Theme") {
                SettingsRow("Light Theme") {
                    themeButton(
                        title: currentLightTheme ?? "Default",
                        isPresented: $showLightThemePicker,
                        mode: .light
                    )
                }
                SettingsRow("Dark Theme") {
                    themeButton(
                        title: currentDarkTheme ?? "Default",
                        isPresented: $showDarkThemePicker,
                        mode: .dark
                    )
                }
            }

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
        .task {
            refreshThemeNames()
        }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in
            refreshThemeNames()
        }
    }

    private func themeButton(
        title: String,
        isPresented: Binding<Bool>,
        mode: ThemePickerMode
    ) -> some View {
        Button {
            isPresented.wrappedValue.toggle()
        } label: {
            HStack(spacing: 6) {
                Text(title)
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .lineLimit(1)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.system(size: 10))
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .foregroundStyle(SettingsStyle.foreground)
            .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))
        }
        .buttonStyle(.plain)
        .popover(isPresented: isPresented) {
            ThemePicker(mode: mode)
                .environment(themeService)
        }
    }

    private func refreshThemeNames() {
        currentLightTheme = themeService.currentLightThemeName()
        currentDarkTheme = themeService.currentDarkThemeName()
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
