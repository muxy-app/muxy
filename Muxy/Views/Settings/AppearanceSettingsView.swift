import SwiftUI

struct AppearanceSettingsView: View {
    @State private var themeService = ThemeService.shared
    @State private var showThemePicker = false
    @State private var currentTheme: String?
    @AppStorage("muxy.vcsDisplayMode") private var vcsDisplayMode = VCSDisplayMode.attached.rawValue

    var body: some View {
        SettingsContainer {
            SettingsSection("Terminal") {
                SettingsRow("Theme") {
                    Button {
                        showThemePicker.toggle()
                    } label: {
                        HStack(spacing: 6) {
                            Text(currentTheme ?? "Default")
                                .font(.system(size: SettingsMetrics.labelFontSize))
                                .lineLimit(1)
                            Image(systemName: "chevron.up.chevron.down")
                                .font(.system(size: 10))
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
                    }
                    .buttonStyle(.plain)
                    .popover(isPresented: $showThemePicker) {
                        ThemePicker()
                            .environment(themeService)
                    }
                }
            }

            SettingsSection("Source Control", showsDivider: false) {
                SettingsRow("Display Mode") {
                    Picker("", selection: $vcsDisplayMode) {
                        ForEach(VCSDisplayMode.allCases) { mode in
                            Text(mode.title).tag(mode.rawValue)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.segmented)
                    .frame(width: SettingsMetrics.controlWidth)
                }
            }
        }
        .task {
            currentTheme = themeService.currentThemeName()
        }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in
            currentTheme = themeService.currentThemeName()
        }
    }
}
