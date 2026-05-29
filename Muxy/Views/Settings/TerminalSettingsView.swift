import SwiftUI

struct TerminalSettingsView: View {
    @State private var themeService = ThemeService.shared
    @State private var showLightThemePicker = false
    @State private var showDarkThemePicker = false
    @State private var currentLightTheme: String?
    @State private var currentDarkTheme: String?
    @AppStorage(GeneralSettingsKeys.autoCopyTerminalSelection)
    private var autoCopyTerminalSelection = false
    @AppStorage(TabCloseConfirmationPreferences.confirmRunningProcessKey)
    private var confirmRunningProcess = true
    @AppStorage(SessionRestorePreferences.enabledKey)
    private var restoreSessionsEnabled = SessionRestorePreferences.defaultIsEnabled
    @AppStorage(GeneralSettingsKeys.lowMemoryMode)
    private var lowMemoryMode = false
    @State private var excludedCommands = SessionRestorePreferences.excludedCommandsText
    private var tmuxInstalled: Bool = {
        let candidates = [
            "/opt/homebrew/bin/tmux",
            "/usr/local/bin/tmux",
            "/opt/local/bin/tmux",
            "/usr/bin/tmux",
        ]
        return candidates.contains { FileManager.default.isExecutableFile(atPath: $0) }
    }()

    var body: some View {
        SettingsContainer {
            SettingsSection("Appearance") {
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

            SettingsSection(
                "Selection",
                footer: "When enabled, releasing the mouse after selecting text in the terminal copies it to the clipboard."
            ) {
                SettingsToggleRow(
                    label: "Auto-copy selected text",
                    isOn: $autoCopyTerminalSelection
                )
            }

            SettingsSection("Tabs") {
                SettingsToggleRow(
                    label: "Confirm before closing a tab with a running process",
                    isOn: $confirmRunningProcess
                )
            }

            let footer = """
            Reduces RAM by evicting hidden terminal surfaces. \
            Requires tmux (brew install tmux).

            • Shell state persists across workspace switches
            • Smooth scrolling replaced with tmux-style scroll
            • Text selection handled by tmux instead of native Ghostty
            • Existing terminals require reopening to apply
            """
            SettingsSection("Performance", footer: footer) {
                SettingsToggleRow(
                    label: "Low Memory Mode",
                    isOn: $lowMemoryMode
                )
                .disabled(!tmuxInstalled)
                if !tmuxInstalled {
                    HStack {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(.yellow)
                        Text("tmux not found — install with `brew install tmux`")
                            .font(.system(size: SettingsMetrics.labelFontSize))
                            .foregroundStyle(MuxyTheme.fgMuted)
                    }
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                }
            }

            SettingsSection(
                "Session Restore",
                footer: "Sessions are restored when a project is opened for the first time after launch."
            ) {
                SettingsToggleRow(
                    label: "Restore terminal sessions",
                    isOn: $restoreSessionsEnabled
                )
            }

            SettingsSection(
                "Blocked Commands",
                footer: "One command or prefix per line. Matching commands are never started automatically.",
                showsDivider: false
            ) {
                HStack {
                    Spacer()
                    Button("Reset to Defaults") {
                        excludedCommands = SessionRestorePreferences.defaultExcludedCommands.joined(separator: "\n")
                        SessionRestorePreferences.excludedCommandsText = excludedCommands
                    }
                    .fixedSize(horizontal: true, vertical: false)
                    .disabled(excludedCommands == SessionRestorePreferences.defaultExcludedCommands.joined(separator: "\n"))
                }
                .padding(.horizontal, SettingsMetrics.horizontalPadding)
                TextEditor(text: $excludedCommands)
                    .font(.system(size: 12, design: .monospaced))
                    .scrollContentBackground(.hidden)
                    .settingsTextInput(minHeight: 180)
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                    .onChange(of: excludedCommands) { _, value in
                        SessionRestorePreferences.excludedCommandsText = value
                    }
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
