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
    @State private var defaultWorktreeLocation = WorktreeLocationSelection()

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
                footer: "Templates must include {branch}; {project-name} and {base-dir} are optional. Relative templates "
                    + "start from the project folder. Folder mode keeps the existing project and worktree subfolder layout.",
                showsDivider: false
            ) {
                worktreeLocationControl
            }
        }
        .task {
            loadDefaultWorktreeLocation()
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

    private var defaultWorktreeLocationMode: Binding<WorktreeLocationMode> {
        Binding(
            get: { defaultWorktreeLocation.mode },
            set: { mode in
                var selection = defaultWorktreeLocation
                selection.select(mode)
                defaultWorktreeLocation = selection
                persistDefaultWorktreeLocation(selection)
            }
        )
    }

    private var defaultWorktreeLocationValue: Binding<String> {
        Binding(
            get: { defaultWorktreeLocation.value },
            set: { value in
                var selection = defaultWorktreeLocation
                selection.value = value
                defaultWorktreeLocation = selection
                persistDefaultWorktreeLocation(selection)
            }
        )
    }

    private var worktreeLocationControl: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Default worktree location")
                    .font(.system(size: SettingsMetrics.labelFontSize))
                Spacer()
                Picker("", selection: defaultWorktreeLocationMode) {
                    Text("App Default").tag(WorktreeLocationMode.defaultLocation)
                    Text("Template").tag(WorktreeLocationMode.pathTemplate)
                    Text("Folder").tag(WorktreeLocationMode.parentFolder)
                }
                .labelsHidden()
                .pickerStyle(.segmented)
                .frame(width: SettingsMetrics.controlWidth)
            }

            worktreeLocationValueControl

            if let message = defaultWorktreeLocationValidationMessage {
                Text(message)
                    .font(.system(size: SettingsMetrics.footnoteFontSize))
                    .foregroundStyle(SettingsStyle.destructive)
            }
        }
        .padding(.horizontal, SettingsMetrics.horizontalPadding)
        .padding(.vertical, SettingsMetrics.rowVerticalPadding)
    }

    @ViewBuilder
    private var worktreeLocationValueControl: some View {
        switch defaultWorktreeLocation.mode {
        case .defaultLocation:
            Text("Muxy App Support")
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .frame(maxWidth: .infinity, alignment: .leading)
        case .pathTemplate:
            TextField(WorktreeLocationResolver.suggestedPathTemplate, text: defaultWorktreeLocationValue)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .settingsTextInput(maxWidth: .infinity, minHeight: 22)
        case .parentFolder:
            HStack(spacing: 8) {
                TextField("/path/to/worktrees", text: defaultWorktreeLocationValue)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                    .settingsTextInput(maxWidth: .infinity, minHeight: 22)

                Button("Choose Folder...") {
                    chooseDefaultWorktreeParentPath()
                }
                .fixedSize(horizontal: true, vertical: false)
            }
        }
    }

    private var defaultWorktreeLocationValidationMessage: String? {
        let message: String? = switch defaultWorktreeLocation.mode {
        case .defaultLocation:
            nil
        case .pathTemplate:
            WorktreeLocationResolver.pathTemplateValidationMessage(defaultWorktreeLocation.value)
        case .parentFolder:
            WorktreeLocationResolver.normalizedLocation(defaultWorktreeLocation.value) == nil
                ? "Folder is required."
                : nil
        }
        guard let message else { return nil }
        return "\(message) \(persistedDefaultWorktreeLocationDescription) remains active."
    }

    private var persistedDefaultWorktreeLocationDescription: String {
        if let template = WorktreeLocationResolver.normalizedLocation(defaultWorktreePathTemplate) {
            return "Saved template \(template)"
        }
        if let folder = WorktreeLocationResolver.normalizedLocation(defaultWorktreeParentPath) {
            return "Saved folder \(folder)"
        }
        return "App Default"
    }

    private func chooseDefaultWorktreeParentPath() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = "Select the default folder for new worktrees"
        if let path = WorktreeLocationResolver.normalizedLocation(defaultWorktreeLocation.parentPath) {
            panel.directoryURL = URL(fileURLWithPath: path, isDirectory: true)
        }
        guard panel.runModal() == .OK, let url = panel.url else { return }
        var selection = defaultWorktreeLocation
        selection.select(.parentFolder)
        selection.value = url.path
        defaultWorktreeLocation = selection
        persistDefaultWorktreeLocation(selection)
    }

    private func loadDefaultWorktreeLocation() {
        defaultWorktreeLocation = WorktreeLocationSelection(
            pathTemplate: defaultWorktreePathTemplate,
            parentPath: defaultWorktreeParentPath
        )
    }

    private func persistDefaultWorktreeLocation(_ selection: WorktreeLocationSelection) {
        switch selection.mode {
        case .defaultLocation:
            defaultWorktreePathTemplate = ""
            defaultWorktreeParentPath = ""
        case .pathTemplate:
            guard let template = try? WorktreeLocationResolver.validatedPathTemplate(selection.value) else { return }
            defaultWorktreePathTemplate = template
            defaultWorktreeParentPath = ""
        case .parentFolder:
            guard let folder = WorktreeLocationResolver.normalizedLocation(selection.value) else { return }
            defaultWorktreeParentPath = folder
            defaultWorktreePathTemplate = ""
        }
    }
}
