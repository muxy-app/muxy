import SwiftUI

struct RepositoryAIActionProjectPromptConfiguration {
    let projectName: String
    let prompt: String?
    let fallbackPrompt: String
    let onSave: (String?) -> Void
}

struct RepositoryAIActionSplitButton: View {
    let action: RepositoryAIAction
    let providers: [any AIAgentLaunchProvider]
    let selectedProvider: (any AIAgentLaunchProvider)?
    let installedProviderIDs: Set<String>
    let isRemote: Bool
    let availability: RepositoryAIActionAvailability
    let isRunning: Bool
    let menuDisabled: Bool
    @Binding var configuredProviderID: String
    let projectPrompt: RepositoryAIActionProjectPromptConfiguration?
    let onRun: () -> Void

    @State private var hoveredPrimary = false
    @State private var hoveredMenu = false
    @State private var showingMenu = false
    @State private var editingProjectPrompt = false
    @State private var projectPromptDraft = ""

    var body: some View {
        HStack(spacing: 0) {
            primaryButton
            menuButton
        }
        .fixedSize(horizontal: true, vertical: false)
        .background(
            showingMenu ? MuxyTheme.surface : Color.clear,
            in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
        )
        .popover(isPresented: $showingMenu, arrowEdge: .bottom) {
            popoverContent
        }
        .onChange(of: showingMenu) { _, isShowing in
            if !isShowing {
                editingProjectPrompt = false
            }
        }
    }

    @ViewBuilder
    private var popoverContent: some View {
        if editingProjectPrompt, let projectPrompt {
            projectPromptEditor(projectPrompt)
        } else {
            providerMenu
        }
    }

    private var primaryButton: some View {
        Button(action: onRun) {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: action.symbolName)
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                Text(isRunning ? action.runningTitle : action.title)
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
            }
            .foregroundStyle(primaryForeground)
            .padding(.leading, UIMetrics.spacing3)
            .padding(.trailing, UIMetrics.spacing2)
            .frame(height: UIMetrics.controlSmall)
            .contentShape(Rectangle())
            .background(primaryBackground, in: UnevenRoundedRectangle(
                topLeadingRadius: UIMetrics.radiusSM,
                bottomLeadingRadius: UIMetrics.radiusSM
            ))
        }
        .buttonStyle(.plain)
        .disabled(!canRun)
        .onHover { hoveredPrimary = $0 }
        .help(primaryHelp)
        .accessibilityLabel(primaryHelp)
    }

    private var menuButton: some View {
        Button {
            showingMenu.toggle()
        } label: {
            Image(systemName: "chevron.down")
                .font(.system(size: UIMetrics.fontMicro, weight: .bold))
                .foregroundStyle(menuForeground)
                .frame(width: UIMetrics.scaled(16), height: UIMetrics.controlSmall)
                .contentShape(Rectangle())
                .background(menuBackground, in: UnevenRoundedRectangle(
                    bottomTrailingRadius: UIMetrics.radiusSM,
                    topTrailingRadius: UIMetrics.radiusSM
                ))
        }
        .buttonStyle(.plain)
        .disabled(menuDisabled)
        .onHover { hoveredMenu = $0 }
        .help("Choose the AI provider for \(action.title)")
        .accessibilityLabel("Choose the AI provider for \(action.title)")
    }

    private var providerMenu: some View {
        VStack(alignment: .leading, spacing: UIMetrics.scaled(1)) {
            Text(action.settingsTitle)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.spacing3)
                .padding(.bottom, UIMetrics.spacing2)

            providerRow(
                id: RepositoryAIActionPreferences.automaticProviderID,
                title: automaticProviderTitle,
                iconName: nil
            )

            Divider().padding(.vertical, UIMetrics.spacing2)

            ForEach(providers, id: \.id) { provider in
                providerRow(
                    id: provider.id,
                    title: providerTitle(provider),
                    iconName: provider.iconName
                )
            }

            if let projectPrompt {
                Divider().padding(.vertical, UIMetrics.spacing2)
                projectPromptRow(projectPrompt)
            }
        }
        .padding(UIMetrics.spacing4)
        .fixedSize(horizontal: true, vertical: true)
        .background(MuxyTheme.bg)
    }

    private func projectPromptRow(
        _ configuration: RepositoryAIActionProjectPromptConfiguration
    ) -> some View {
        Button {
            projectPromptDraft = configuration.prompt ?? configuration.fallbackPrompt
            editingProjectPrompt = true
        } label: {
            HStack(spacing: UIMetrics.spacing3) {
                Image(systemName: "text.quote")
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
                VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                    Text("Edit Project Prompt…")
                        .font(.system(size: UIMetrics.fontBody))
                        .foregroundStyle(MuxyTheme.fg)
                    Text(configuration.projectName)
                        .font(.system(size: UIMetrics.fontCaption))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .lineLimit(1)
                }
                Spacer(minLength: UIMetrics.spacing6)
                if configuration.prompt != nil {
                    Circle()
                        .fill(MuxyTheme.accent)
                        .frame(width: UIMetrics.scaled(6), height: UIMetrics.scaled(6))
                }
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .frame(minWidth: UIMetrics.scaled(220))
            .frame(height: UIMetrics.scaled(44))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func projectPromptEditor(
        _ configuration: RepositoryAIActionProjectPromptConfiguration
    ) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing4) {
            VStack(alignment: .leading, spacing: UIMetrics.spacing1) {
                Text("Create PR Prompt")
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                Text(configuration.projectName)
                    .font(.system(size: UIMetrics.fontCaption))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }

            TextEditor(text: $projectPromptDraft)
                .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                .foregroundStyle(MuxyTheme.fg)
                .scrollContentBackground(.hidden)
                .padding(UIMetrics.spacing3)
                .frame(height: UIMetrics.scaled(150))
                .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay {
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .stroke(MuxyTheme.border, lineWidth: 1)
                }

            Text("This prompt overrides Settings → AI only for this project.")
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)

            HStack(spacing: UIMetrics.spacing3) {
                Button("Use Global Prompt") {
                    configuration.onSave(nil)
                    showingMenu = false
                }
                .buttonStyle(.plain)
                .foregroundStyle(MuxyTheme.accent)
                .disabled(configuration.prompt == nil)
                Spacer(minLength: 0)
                Button("Cancel") {
                    editingProjectPrompt = false
                }
                Button("Save") {
                    configuration.onSave(projectPromptDraft)
                    showingMenu = false
                }
                .keyboardShortcut(.defaultAction)
                .disabled(RepositoryAIActionPreferences.normalizedPrompt(projectPromptDraft) == nil)
            }
            .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
        }
        .padding(UIMetrics.spacing5)
        .frame(width: UIMetrics.scaled(380))
        .background(MuxyTheme.bg)
    }

    private func providerRow(id: String, title: String, iconName: String?) -> some View {
        Button {
            configuredProviderID = id
            showingMenu = false
        } label: {
            HStack(spacing: UIMetrics.spacing3) {
                if let iconName {
                    ProviderIconView(iconName: iconName, size: UIMetrics.iconMD)
                } else {
                    Image(systemName: "sparkles")
                        .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                        .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
                }
                Text(title)
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fg)
                Spacer(minLength: UIMetrics.spacing6)
                Image(systemName: "checkmark")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .bold))
                    .foregroundStyle(MuxyTheme.accent)
                    .opacity(configuredProviderID == id ? 1 : 0)
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .frame(minWidth: UIMetrics.scaled(220))
            .frame(height: UIMetrics.controlMedium)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var canRun: Bool {
        guard availability == .available, selectedProvider != nil else { return false }
        guard let selectedProvider else { return false }
        return isRemote || installedProviderIDs.contains(selectedProvider.id)
    }

    private var primaryHelp: String {
        if isRunning {
            return "\(action.settingsTitle) is running."
        }
        if case let .disabled(reason) = availability {
            return reason
        }
        guard let selectedProvider else {
            return "Install a supported AI provider CLI or choose a provider."
        }
        guard isRemote || installedProviderIDs.contains(selectedProvider.id) else {
            return "\(selectedProvider.displayName) CLI is not installed. Choose another provider or install its CLI."
        }
        return "\(action.settingsTitle) with \(selectedProvider.displayName)"
    }

    private var primaryForeground: Color {
        canRun || isRunning ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private var menuForeground: Color {
        menuDisabled ? MuxyTheme.fgDim : MuxyTheme.fgMuted
    }

    private var primaryBackground: Color {
        hoveredPrimary && canRun ? MuxyTheme.hover : .clear
    }

    private var menuBackground: Color {
        hoveredMenu && !menuDisabled ? MuxyTheme.hover : .clear
    }

    private var automaticProviderTitle: String {
        guard let selectedProvider else { return "Auto" }
        return "Auto · \(selectedProvider.displayName)"
    }

    private func providerTitle(_ provider: any AIAgentLaunchProvider) -> String {
        guard !isRemote, !installedProviderIDs.contains(provider.id) else {
            return provider.displayName
        }
        return "\(provider.displayName) · Not installed"
    }
}
