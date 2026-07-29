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
                            Text(L10n.resource(key: mode.label)).tag(mode.rawValue)
                        }
                    }
                    .labelsHidden()
                    .settingsControl()
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
                            Text(L10n.resource(key: mode.title)).tag(mode.rawValue)
                        }
                    }
                    .labelsHidden()
                    .settingsControl()
                }

                SettingsToggleRow(
                    label: L10n.resource("Keep projects open after closing the last tab"),
                    isOn: $keepProjectsOpenWhenNoTabs
                )
            }

            SettingsSection(
                "Open Files With",
                footer: fileOpenerFooter
            ) {
                SettingsRow("Default Opener") {
                    Picker("", selection: $defaultFileOpener) {
                        ForEach(fileOpenerOptions) { option in
                            if option.id == FileOpenerSelection.builtinValue {
                                Text(L10n.resource(key: option.title))
                                    .tag(option.id)
                            } else {
                                Text(verbatim: option.title)
                                    .tag(option.id)
                                    .disabled(!option.isAvailable)
                            }
                        }
                    }
                    .labelsHidden()
                    .settingsControl()
                }
            }

            SettingsSection(
                "Worktrees",
                footer: """
                Templates must include {branch}; {project-name} and {base-dir} are optional. Relative templates start \
                from the project folder. Folder mode keeps the existing project and worktree subfolder layout.
                """,
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

    private var fileOpenerOptions: [FileOpenerSelection.Option] {
        FileOpenerSelection.options(from: fileOpeners, selectedValue: defaultFileOpener)
    }

    private var fileOpenerFooter: LocalizedStringResource {
        if fileOpenerOptions.contains(where: { $0.id == defaultFileOpener && !$0.isAvailable }) {
            return """
            The selected extension opener is unavailable, so terminal file links currently use the project target \
            selected separately in the top bar.
            """
        }
        return """
        Terminal file links use this opener. Built-in and unmatched extension files use the project target selected \
        separately in the top bar.
        """
    }

    private var projectPickerMode: ProjectPickerMode {
        ProjectPickerMode(rawValue: projectPickerModeRaw) ?? .custom
    }

    private var projectsFooter: LocalizedStringResource {
        if projectPickerMode == .custom {
            return """
            Muxy Picker searches this location by folder name. Use App Default to search your home folder. Projects \
            can stay in the sidebar after closing their last tab.
            """
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
            HStack(spacing: 0) {
                Text(L10n.resource("Default worktree location"))
                    .font(.system(size: SettingsMetrics.labelFontSize))
                    .lineLimit(2)
                    .truncationMode(.tail)
                    .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: SettingsMetrics.rowSpacing)
                Picker("", selection: defaultWorktreeLocationMode) {
                    Text(L10n.resource("App Default")).tag(WorktreeLocationMode.defaultLocation)
                    Text(L10n.resource("Template")).tag(WorktreeLocationMode.pathTemplate)
                    Text(L10n.resource("Folder")).tag(WorktreeLocationMode.parentFolder)
                }
                .labelsHidden()
                .pickerStyle(.segmented)
                .settingsControl(.intrinsic)
                .layoutPriority(1)
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
            Text(L10n.resource("Muxy App Support"))
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .foregroundStyle(SettingsStyle.mutedForeground)
                .frame(maxWidth: .infinity, alignment: .leading)
        case .pathTemplate:
            TextField(WorktreeLocationResolver.suggestedPathTemplate, text: defaultWorktreeLocationValue)
                .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                .settingsTextInput(maxWidth: .infinity, minHeight: 22)
        case .parentFolder:
            HStack(spacing: 8) {
                TextField(L10n.string("/path/to/worktrees"), text: defaultWorktreeLocationValue)
                    .font(.system(size: SettingsMetrics.footnoteFontSize, design: .monospaced))
                    .settingsTextInput(maxWidth: .infinity, minHeight: 22)

                Button(L10n.string("Choose Folder...")) {
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
        let localizedMessage = L10n.string(key: message)
        return L10n.string("\(localizedMessage) \(persistedDefaultWorktreeLocationDescription) remains active.")
    }

    private var persistedDefaultWorktreeLocationDescription: String {
        if let template = WorktreeLocationResolver.normalizedLocation(defaultWorktreePathTemplate) {
            return L10n.string("Saved template \(template)")
        }
        if let folder = WorktreeLocationResolver.normalizedLocation(defaultWorktreeParentPath) {
            return L10n.string("Saved folder \(folder)")
        }
        return L10n.string("App Default")
    }

    private func chooseDefaultWorktreeParentPath() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.message = L10n.string("Select the default folder for new worktrees")
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
