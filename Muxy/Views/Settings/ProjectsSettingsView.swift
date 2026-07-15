import AppKit
import SwiftUI

struct ProjectsSettingsView: View {
    @AppStorage(GeneralSettingsKeys.defaultWorktreePathTemplate)
    private var defaultWorktreePathTemplate = ""
    @AppStorage(GeneralSettingsKeys.defaultWorktreeParentPath)
    private var defaultWorktreeParentPath = ""
    @AppStorage(ProjectLifecyclePreferences.keepOpenWhenNoTabsKey)
    private var keepProjectsOpenWhenNoTabs = false
    @AppStorage(ProjectPickerPreferences.storageKey)
    private var projectPickerModeRaw = ProjectPickerMode.custom.rawValue
    @AppStorage(ProjectSortMode.storageKey)
    private var projectSortModeRaw = ProjectSortMode.defaultValue.rawValue
    @AppStorage(FileOpenerSelection.storageKey)
    private var defaultFileOpener = FileOpenerSelection.builtinValue
    @State private var projectPickerDefaultLocationSettings = ProjectPickerDefaultLocationSettingsModel()
    @State private var extensionStore = ExtensionStore.shared

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

                SettingsRow("Sort Projects By") {
                    Picker("", selection: $projectSortModeRaw) {
                        ForEach(ProjectSortMode.allCases) { mode in
                            Text(mode.title).tag(mode.rawValue)
                        }
                    }
                    .labelsHidden()
                    .frame(width: SettingsMetrics.controlWidth, alignment: .trailing)
                }

                SettingsToggleRow(
                    label: "Keep projects open after closing the last tab",
                    isOn: $keepProjectsOpenWhenNoTabs
                )
            }

            if !fileOpeners.isEmpty {
                SettingsSection(
                    "Open Files With",
                    footer: "Files opened from the terminal or the Open in IDE control go to this opener. "
                        + "Falls back to the IDE when its patterns don't match."
                ) {
                    SettingsRow("Default Opener") {
                        HStack {
                            Spacer()
                            Picker("", selection: $defaultFileOpener) {
                                Text("Built-in (IDE)").tag(FileOpenerSelection.builtinValue)
                                ForEach(fileOpeners) { binding in
                                    Text(label(for: binding)).tag(binding.id)
                                }
                            }
                            .labelsHidden()
                            .fixedSize()
                        }
                        .frame(width: SettingsMetrics.controlWidth)
                    }
                }
            }

            SettingsSection(
                "Worktrees",
                footer: "Use {project-name}, {base-dir}, and {branch} in a path template. Relative templates start "
                    + "from the project folder. Choose Folder keeps the existing project and worktree subfolder layout.",
                showsDivider: false
            ) {
                worktreeLocationControl
            }
        }
    }

    private var fileOpeners: [ExtensionStore.FileOpenerBinding] {
        FileOpenerSelection.availableOpeners(store: extensionStore)
    }

    private func label(for binding: ExtensionStore.FileOpenerBinding) -> String {
        guard let title = binding.opener.title, !title.isEmpty else {
            return binding.muxyExtension.displayName
        }
        return "\(binding.muxyExtension.displayName) (\(title))"
    }

    private var projectPickerMode: ProjectPickerMode {
        ProjectPickerMode(rawValue: projectPickerModeRaw) ?? .custom
    }

    private var projectsFooter: String {
        if projectPickerMode == .custom {
            return "Muxy Picker searches this location by folder name. Use App Default to search your home folder. "
                + "Projects can stay in the sidebar after closing their last tab."
        }
        return "Muxy Picker can use Finder or Muxy's picker. Projects can stay in the sidebar after closing their last tab."
    }

    private var defaultWorktreeLocation: Binding<String> {
        Binding(
            get: {
                if !defaultWorktreePathTemplate.isEmpty {
                    return defaultWorktreePathTemplate
                }
                return defaultWorktreeParentPath
            },
            set: { value in
                defaultWorktreePathTemplate = value
                defaultWorktreeParentPath = ""
            }
        )
    }

    private var usesAppDefaultWorktreeLocation: Bool {
        defaultWorktreePathTemplate.isEmpty && defaultWorktreeParentPath.isEmpty
    }

    private var worktreeLocationControl: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Default path template for new worktrees")
                .font(.system(size: SettingsMetrics.labelFontSize))

            HStack(alignment: .center, spacing: 8) {
                pathDisplay
                    .layoutPriority(1)

                Button("Choose Folder...") {
                    chooseDefaultWorktreeParentPath()
                }
                .fixedSize(horizontal: true, vertical: false)

                Button("Use App Default") {
                    defaultWorktreePathTemplate = ""
                    defaultWorktreeParentPath = ""
                }
                .fixedSize(horizontal: true, vertical: false)
                .disabled(usesAppDefaultWorktreeLocation)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    private var pathDisplay: some View {
        TextField("Muxy App Support", text: defaultWorktreeLocation)
            .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
            .settingsTextInput(maxWidth: .infinity, minHeight: 22)
            .frame(minWidth: 170)
    }

    private func chooseDefaultWorktreeParentPath() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select the default folder for new worktrees"
        if let path = WorktreeLocationResolver.normalizedLocation(defaultWorktreeParentPath) {
            panel.directoryURL = URL(fileURLWithPath: path, isDirectory: true)
        }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        defaultWorktreePathTemplate = ""
        defaultWorktreeParentPath = url.path
    }
}
