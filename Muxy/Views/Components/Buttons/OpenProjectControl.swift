import SwiftUI

@MainActor
struct OpenProjectControl: View {
    let projectPath: String?

    @ObservedObject private var ideService = IDEIntegrationService.shared
    @State private var hoveredPrimary = false
    @State private var hoveredMenu = false
    @State private var showingMenu = false

    var body: some View {
        HStack(spacing: 0) {
            Button(action: openProject) {
                HStack(spacing: UIMetrics.spacing3) {
                    primaryIcon
                    Text(defaultIDE?.displayName ?? "Open Project")
                        .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .frame(maxWidth: UIMetrics.scaled(112), alignment: .leading)
                }
                .foregroundStyle(primaryForeground)
                .padding(.horizontal, UIMetrics.spacing4)
                .frame(height: UIMetrics.controlSmall)
                .contentShape(Rectangle())
                .background(hoveredPrimary ? MuxyTheme.hover : .clear)
            }
            .buttonStyle(.plain)
            .disabled(projectPath == nil || defaultIDE == nil)
            .onHover { hoveredPrimary = $0 }
            .help(helpText)
            .accessibilityLabel(helpText)

            Rectangle()
                .fill(MuxyTheme.border)
                .frame(width: UIMetrics.scaled(1), height: UIMetrics.scaled(14))

            menuToggleButton
        }
        .fixedSize(horizontal: true, vertical: false)
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous)
                .strokeBorder(MuxyTheme.border, lineWidth: UIMetrics.scaled(1))
        }
        .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD, style: .continuous))
        .padding(.trailing, UIMetrics.spacing2)
        .popover(isPresented: $showingMenu, arrowEdge: .bottom) {
            menuPopoverContent
        }
    }

    @ViewBuilder
    private var primaryIcon: some View {
        if let defaultIDE {
            AppBundleIconView(
                appURL: defaultIDE.appURL,
                fallbackSystemName: defaultIDE.symbolName,
                size: UIMetrics.iconMD
            )
        } else {
            Image(systemName: "chevron.left.forwardslash.chevron.right")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
        }
    }

    private var menuToggleButton: some View {
        Button {
            guard projectPath != nil else { return }
            showingMenu.toggle()
        } label: {
            Image(systemName: "chevron.down")
                .font(.system(size: UIMetrics.fontMicro, weight: .semibold))
                .foregroundStyle(menuForeground)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlSmall)
                .contentShape(Rectangle())
                .background(hoveredMenu || showingMenu ? MuxyTheme.hover : .clear)
        }
        .buttonStyle(.plain)
        .disabled(projectPath == nil)
        .onHover { hoveredMenu = $0 }
        .help(menuHelpText)
        .accessibilityLabel(menuHelpText)
    }

    private var menuPopoverContent: some View {
        VStack(alignment: .leading, spacing: 0) {
            if projectPath != nil {
                menuActionRow(
                    appURL: IDEIntegrationService.finderAppURL,
                    fallbackSystemName: "folder",
                    title: "Finder"
                ) {
                    showingMenu = false
                    ideService.selectProjectTarget(IDEIntegrationService.finderApplication)
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

    private var hasTargets: Bool {
        !installedApps.isEmpty
    }

    private var defaultIDE: IDEIntegrationService.IDEApplication? {
        ideService.defaultIDE
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
                ideService.selectProjectTarget(ide)
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
        guard projectPath != nil else { return "Open a project to enable opening" }
        if let defaultIDE {
            return "Open project in \(defaultIDE.displayName)"
        }
        return hasTargets ? "No default editor available" : "No supported editors found"
    }

    private var menuHelpText: String {
        guard projectPath != nil else { return "Open a project to choose a target" }
        return "Choose project target"
    }

    private var primaryForeground: Color {
        if projectPath == nil || defaultIDE == nil {
            return MuxyTheme.fgDim
        }
        return MuxyTheme.fg
    }

    private var menuForeground: Color {
        if projectPath == nil {
            return MuxyTheme.fgDim
        }
        return hoveredMenu || showingMenu ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private func openProject() {
        guard let projectPath, let defaultIDE else { return }
        _ = ideService.openProject(at: projectPath, in: defaultIDE)
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
