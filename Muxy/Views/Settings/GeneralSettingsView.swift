import SwiftUI

enum GeneralSettingsKeys {
    static let autoExpandWorktreesOnProjectSwitch = "muxy.general.autoExpandWorktreesOnProjectSwitch"
}

struct GeneralSettingsView: View {
    @AppStorage(GeneralSettingsKeys.autoExpandWorktreesOnProjectSwitch)
    private var autoExpandWorktrees = false

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Sidebar",
                footer: "Automatically reveal worktrees when you switch to a project.",
                showsDivider: false
            ) {
                SettingsToggleRow(
                    label: "Auto-expand worktrees on project switch",
                    isOn: $autoExpandWorktrees
                )
            }
        }
    }
}
