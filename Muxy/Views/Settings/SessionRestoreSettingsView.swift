import SwiftUI

struct SessionRestoreSettingsView: View {
    @AppStorage(SessionRestorePreferences.enabledKey) private var enabled = true
    @State private var excludedCommands = SessionRestorePreferences.excludedCommandsText

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Restore",
                footer: "Sessions are restored when a project is opened for the first time after launch."
            ) {
                SettingsToggleRow(
                    label: "Restore terminal sessions",
                    isOn: $enabled
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
                    .frame(minHeight: 180)
                    .padding(8)
                    .background(MuxyTheme.bg)
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(MuxyTheme.border, lineWidth: 1)
                    )
                    .padding(.horizontal, SettingsMetrics.horizontalPadding)
                    .padding(.vertical, SettingsMetrics.rowVerticalPadding)
                    .onChange(of: excludedCommands) { _, value in
                        SessionRestorePreferences.excludedCommandsText = value
                    }
            }
        }
    }
}
