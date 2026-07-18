import SwiftUI

@MainActor
struct OpenInIDEControl: View {
    let projectPath: String?
    var projectID: UUID?
    var areaID: UUID?
    var compact = true

    @Environment(AppState.self) private var appState
    @ObservedObject private var ideService = IDEIntegrationService.shared
    @State private var extensionStore = ExtensionStore.shared
    @AppStorage(FileOpenerSelection.storageKey) private var selectedFileOpenerValue = FileOpenerSelection.builtinValue
    @State private var hoveredPrimary = false
    @State private var hoveredMenu = false
    @State private var showingMenu = false

    private enum OpenTarget {
        case ide(IDEIntegrationService.IDEApplication)
        case fileOpener(ExtensionStore.FileOpenerBinding)
    }

    var body: some View {
        if compact {
            compactSplitButton
        } else {
            expandedSplitButton
        }
    }

    private var compactSplitButton: some View {
        HStack(spacing: 0) {
            Button(action: openDefaultIDE) {
                Group {
                    if let defaultIDE = defaultTargetIDE {
                        AppBundleIconView(appURL: defaultIDE.appURL, fallbackSystemName: defaultIDE.symbolName, size: UIMetrics.iconLG)
                    } else if defaultFileOpener != nil {
                        Image(systemName: "doc.text")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                            .foregroundStyle(primaryForeground)
                    } else {
                        Image(systemName: "chevron.left.forwardslash.chevron.right")
                            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                            .foregroundStyle(primaryForeground)
                    }
                }
                .frame(width: UIMetrics.scaled(22), height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
                .background(hoveredPrimary ? MuxyTheme.hover : .clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            }
            .buttonStyle(.plain)
            .disabled(projectPath == nil || defaultOpenTarget == nil)
            .onHover { hoveredPrimary = $0 }
            .help(helpText)
            .accessibilityLabel(helpText)

            menuToggleButton(width: 14)
        }
        .popover(isPresented: $showingMenu, arrowEdge: .bottom) {
            menuPopoverContent
        }
    }

    private var expandedSplitButton: some View {
        HStack(spacing: 0) {
            Button(action: openDefaultIDE) {
                HStack(spacing: UIMetrics.spacing3) {
                    if let defaultIDE = defaultTargetIDE {
                        AppBundleIconView(appURL: defaultIDE.appURL, fallbackSystemName: defaultIDE.symbolName, size: UIMetrics.iconLG)
                    } else if defaultFileOpener != nil {
                        Image(systemName: "doc.text")
                    } else {
                        Image(systemName: "chevron.left.forwardslash.chevron.right")
                    }
                    Text(defaultOpenTarget.map { "Open in \(displayName(for: $0))" } ?? "Open in IDE")
                }
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(primaryForeground)
                .padding(.horizontal, UIMetrics.spacing4)
                .frame(height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
                .background(hoveredPrimary ? MuxyTheme.hover : .clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            }
            .buttonStyle(.plain)
            .disabled(projectPath == nil || defaultOpenTarget == nil)
            .onHover { hoveredPrimary = $0 }
            .help(helpText)
            .accessibilityLabel(helpText)

            menuToggleButton(width: UIMetrics.scaled(18))
        }
        .popover(isPresented: $showingMenu, arrowEdge: .bottom) {
            menuPopoverContent
        }
    }

    private func menuToggleButton(width: CGFloat) -> some View {
        Button {
            guard projectPath != nil else { return }
            showingMenu.toggle()
        } label: {
            Image(systemName: "chevron.down")
                .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
                .foregroundStyle(menuForeground)
                .frame(width: width, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(projectPath == nil)
        .onHover { hoveredMenu = $0 }
        .help(menuHelpText)
    }

    private var menuPopoverContent: some View {
        VStack(alignment: .leading, spacing: 0) {
            if let projectPath {
                menuActionRow(
                    appURL: IDEIntegrationService.finderAppURL,
                    fallbackSystemName: "folder",
                    title: "Finder"
                ) {
                    showingMenu = false
                    _ = ideService.openProject(at: projectPath, in: IDEIntegrationService.finderApplication)
                }
                if hasTargets {
                    Divider()
                        .padding(.vertical, UIMetrics.spacing2)
                }
            }

            if !hasTargets {
                Text("No supported editors found")
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .padding(.leading, UIMetrics.spacing5)
                    .padding(.trailing, UIMetrics.spacing6)
                    .padding(.vertical, UIMetrics.spacing4)
            } else {
                if !fileOpeners.isEmpty {
                    fileOpenerSection(title: "Muxy Editors", openers: fileOpeners)
                }
                if !editorApps.isEmpty {
                    menuSection(title: "Editors & IDEs", apps: editorApps)
                }
                if !otherToolApps.isEmpty {
                    menuSection(title: "Other Tools", apps: otherToolApps)
                }
            }
        }
        .padding(UIMetrics.spacing4)
        .fixedSize(horizontal: true, vertical: true)
        .background(MuxyTheme.bg)
    }

    private func fileOpenerSection(title: String, openers: [ExtensionStore.FileOpenerBinding]) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
            Text(title)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.leading, UIMetrics.scaled(9))
                .padding(.trailing, UIMetrics.spacing6)
                .padding(.top, UIMetrics.spacing2)
                .padding(.bottom, UIMetrics.scaled(1))

            ForEach(openers, id: \.id) { binding in
                fileOpenerButton(for: binding)
            }
        }
    }

    private func menuSection(title: String, apps: [IDEIntegrationService.IDEApplication]) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
            Text(title)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.leading, UIMetrics.scaled(9))
                .padding(.trailing, UIMetrics.spacing6)
                .padding(.top, UIMetrics.spacing2)
                .padding(.bottom, UIMetrics.scaled(1))

            ForEach(apps) { ide in
                menuButton(for: ide)
            }
        }
    }

    private var installedApps: [IDEIntegrationService.IDEApplication] {
        ideService.installedApps
    }

    private var fileOpeners: [ExtensionStore.FileOpenerBinding] {
        extensionStore.fileOpeners()
            .sorted {
                displayName(for: .fileOpener($0)).localizedCaseInsensitiveCompare(displayName(for: .fileOpener($1))) == .orderedAscending
            }
    }

    private var hasTargets: Bool {
        !installedApps.isEmpty || !fileOpeners.isEmpty
    }

    private var defaultIDE: IDEIntegrationService.IDEApplication? {
        ideService.defaultIDE
    }

    private var defaultFileOpener: ExtensionStore.FileOpenerBinding? {
        FileOpenerSelection.resolvedBinding(from: selectedFileOpenerValue, store: extensionStore)
    }

    private var defaultTargetIDE: IDEIntegrationService.IDEApplication? {
        if defaultFileOpener != nil {
            return nil
        }
        return defaultIDE
    }

    private var defaultOpenTarget: OpenTarget? {
        if let defaultFileOpener {
            return .fileOpener(defaultFileOpener)
        }
        if let defaultIDE {
            return .ide(defaultIDE)
        }
        return nil
    }

    private var editorApps: [IDEIntegrationService.IDEApplication] {
        let apps = installedApps.filter { $0.group == .editor }
        return apps.sorted { $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending }
    }

    private var otherToolApps: [IDEIntegrationService.IDEApplication] {
        let apps = installedApps.filter { $0.group == .otherTool }
        return apps.sorted { $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending }
    }

    private func menuButton(for ide: IDEIntegrationService.IDEApplication) -> some View {
        IDEMenuRow(
            ide: ide,
            action: {
                showingMenu = false
                open(ide)
            }
        )
    }

    private func fileOpenerButton(for binding: ExtensionStore.FileOpenerBinding) -> some View {
        FileOpenerMenuRow(
            title: displayName(for: .fileOpener(binding)),
            isSelected: isSelected(binding),
            action: {
                showingMenu = false
                open(.fileOpener(binding))
            }
        )
    }

    private func menuActionRow(
        appURL: URL,
        fallbackSystemName: String,
        title: String,
        action: @escaping () -> Void
    ) -> some View {
        IDEMenuActionRow(appURL: appURL, fallbackSystemName: fallbackSystemName, title: title, action: action)
    }

    private var helpText: String {
        guard projectPath != nil else { return "Open a project to enable IDE launching" }
        if let defaultOpenTarget {
            return "Open in \(displayName(for: defaultOpenTarget))"
        }
        return hasTargets ? "No default editor available" : "No supported editors found"
    }

    private var menuHelpText: String {
        guard projectPath != nil else { return "Open a project to choose an editor" }
        return "Choose editor"
    }

    private var primaryForeground: Color {
        if projectPath == nil || defaultOpenTarget == nil {
            return MuxyTheme.fgMuted.opacity(0.45)
        }
        return hoveredPrimary ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private var menuForeground: Color {
        if projectPath == nil {
            return MuxyTheme.fgMuted.opacity(0.45)
        }
        return hoveredMenu ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private func openDefaultIDE() {
        guard let defaultOpenTarget else { return }
        open(defaultOpenTarget)
    }

    private func open(_ ide: IDEIntegrationService.IDEApplication) {
        guard let projectPath else { return }
        _ = ideService.openProject(at: projectPath, in: ide)
    }

    private func open(_ target: OpenTarget) {
        switch target {
        case let .ide(ide):
            open(ide)
        case let .fileOpener(binding):
            openFileOpener(binding)
        }
    }

    private func openFileOpener(_ binding: ExtensionStore.FileOpenerBinding) {
        ideService.selectFileOpener(extensionID: binding.muxyExtension.id, openerID: binding.opener.id)
        guard let projectID = projectID ?? appState.activeProjectID else { return }
        appState.dispatch(.createExtensionTab(
            projectID: projectID,
            areaID: areaID,
            request: AppState.CreateExtensionTabRequest(
                extensionID: binding.muxyExtension.id,
                tabTypeID: binding.opener.tabType,
                title: binding.opener.title ?? binding.tabType.title,
                data: .object(["source": .string("open-control")]),
                singleton: binding.opener.singleton
            )
        ))
    }

    private func displayName(for target: OpenTarget) -> String {
        switch target {
        case let .ide(ide):
            return ide.displayName
        case let .fileOpener(binding):
            let extensionName = formattedExtensionName(binding.muxyExtension.displayName)
            if let title = binding.opener.title, !title.isEmpty {
                return "\(extensionName) (\(title))"
            }
            return extensionName
        }
    }

    private func formattedExtensionName(_ name: String) -> String {
        if name == name.lowercased() {
            return name.capitalized
        }
        return name
    }

    private func isSelected(_ binding: ExtensionStore.FileOpenerBinding) -> Bool {
        selectedFileOpenerValue == binding.id
    }
}

@MainActor
private struct IDEMenuRow: View {
    let ide: IDEIntegrationService.IDEApplication
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.scaled(7)) {
                AppBundleIconView(appURL: ide.appURL, fallbackSystemName: ide.symbolName, size: UIMetrics.iconMD)
                Text(ide.displayName)
                    .font(.system(size: UIMetrics.fontBody))
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.leading, UIMetrics.scaled(9))
            .padding(.trailing, UIMetrics.spacing6)
            .padding(.vertical, UIMetrics.spacing2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(hovered ? MuxyTheme.hover : .clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

@MainActor
private struct FileOpenerMenuRow: View {
    let title: String
    let isSelected: Bool
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.scaled(7)) {
                Image(systemName: "doc.text")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
                Text(title)
                    .font(.system(size: UIMetrics.fontBody))
                Spacer(minLength: UIMetrics.spacing4)
                if isSelected {
                    Image(systemName: "checkmark")
                        .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
                }
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.leading, UIMetrics.scaled(9))
            .padding(.trailing, UIMetrics.spacing6)
            .padding(.vertical, UIMetrics.spacing2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(hovered ? MuxyTheme.hover : .clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

@MainActor
private struct IDEMenuActionRow: View {
    let appURL: URL
    let fallbackSystemName: String
    let title: String
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.scaled(7)) {
                AppBundleIconView(appURL: appURL, fallbackSystemName: fallbackSystemName, size: UIMetrics.iconMD)
                Text(title)
                    .font(.system(size: UIMetrics.fontBody))
            }
            .foregroundStyle(MuxyTheme.fg)
            .padding(.leading, UIMetrics.scaled(9))
            .padding(.trailing, UIMetrics.spacing6)
            .padding(.vertical, UIMetrics.spacing2)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(hovered ? MuxyTheme.hover : .clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}
