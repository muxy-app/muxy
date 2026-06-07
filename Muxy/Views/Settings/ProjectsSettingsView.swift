import AppKit
import SwiftUI

struct ProjectsSettingsView: View {
    @AppStorage(GeneralSettingsKeys.defaultWorktreeParentPath)
    private var defaultWorktreeParentPath = ""
    @AppStorage(ProjectLifecyclePreferences.keepOpenWhenNoTabsKey)
    private var keepProjectsOpenWhenNoTabs = false
    @AppStorage(ProjectPickerPreferences.storageKey)
    private var projectPickerModeRaw = ProjectPickerMode.custom.rawValue
    @State private var projectPickerDefaultLocationSettings = ProjectPickerDefaultLocationSettingsModel()

    var body: some View {
        SettingsContainer {
            SettingsSection(
                "Projects",
                footer: projectsFooter
            ) {
                SettingsRow("Muxy Picker") {
                    Picker("", selection: $projectPickerModeRaw) {
                        ForEach(ProjectPickerMode.allCases) { mode in
                            Text(mode.label).tag(mode.rawValue)
                        }
                    }
                    .labelsHidden()
                    .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                }

                if projectPickerMode == .custom {
                    ProjectPickerDefaultLocationSettingsView(
                        model: projectPickerDefaultLocationSettings,
                        pickerModeRaw: projectPickerModeRaw
                    )
                }

                SettingsToggleRow(
                    label: "Keep projects open after closing the last tab",
                    isOn: $keepProjectsOpenWhenNoTabs
                )
            }

            SettingsSection(
                "Worktrees",
                footer: "Muxy creates a project-named subfolder inside this folder. "
                    + "Projects can still override this from the new worktree dialog.",
                showsDivider: false
            ) {
                worktreeLocationControl
            }
        }
    }

    private var projectPickerMode: ProjectPickerMode {
        ProjectPickerMode(rawValue: projectPickerModeRaw) ?? .custom
    }

    private var projectsFooter: String {
        if projectPickerMode == .custom {
            return "Muxy Picker starts in this default location. Use App Default to reset it. "
                + "Projects can stay in the sidebar after closing their last tab."
        }
        return "Muxy Picker can use Finder or Muxy's picker. Projects can stay in the sidebar after closing their last tab."
    }

    private var defaultWorktreeLocationText: String {
        defaultWorktreeParentPath.isEmpty ? "Muxy App Support" : defaultWorktreeParentPath
    }

    private var worktreeLocationControl: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Default path for new worktrees")
                .font(.system(size: SettingsMetrics.labelFontSize))

            HStack(alignment: .center, spacing: 8) {
                pathDisplay
                    .layoutPriority(1)

                Button("Choose Folder...") {
                    chooseDefaultWorktreeParentPath()
                }
                .fixedSize(horizontal: true, vertical: false)

                Button("Use App Default") {
                    defaultWorktreeParentPath = ""
                }
                .fixedSize(horizontal: true, vertical: false)
                .disabled(defaultWorktreeParentPath.isEmpty)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var pathDisplay: some View {
        HStack(spacing: 7) {
            Image(systemName: defaultWorktreeParentPath.isEmpty ? "internaldrive" : "folder")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .frame(width: 15)

            Text(defaultWorktreeLocationText)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(defaultWorktreeParentPath.isEmpty ? SettingsStyle.mutedForeground : SettingsStyle.foreground)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 9)
        .frame(minWidth: 170, maxWidth: .infinity, alignment: .leading)
        .frame(height: 22)
        .background(SettingsStyle.surface, in: RoundedRectangle(cornerRadius: 6))
        .overlay(
            RoundedRectangle(cornerRadius: 6)
                .stroke(SettingsStyle.border, lineWidth: 1)
        )
    }

    private func chooseDefaultWorktreeParentPath() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select the default folder for new worktrees"
        if let path = WorktreeLocationResolver.normalizedPath(defaultWorktreeParentPath) {
            panel.directoryURL = URL(fileURLWithPath: path, isDirectory: true)
        }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        defaultWorktreeParentPath = url.path
    }
}
