import SwiftUI

struct AgentsFocusedLaunchRow: View {
    let project: Project
    let worktree: Worktree
    let onOpenTerminal: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var loadedOptions: LoadedOptions?

    private struct OptionsLoadID: Hashable {
        let projectID: UUID
        let workspaceContext: WorkspaceContext
    }

    private struct LoadedOptions {
        let id: OptionsLoadID
        let options: [AgentTabLaunchOption]
    }

    private var workspaceContext: WorkspaceContext {
        projectGroupStore.workspaceContext(for: project)
    }

    private var optionsLoadID: OptionsLoadID {
        OptionsLoadID(projectID: project.id, workspaceContext: workspaceContext)
    }

    private var launchableOptions: [AgentTabLaunchOption] {
        guard let loadedOptions, loadedOptions.id == optionsLoadID else { return [] }
        return loadedOptions.options.filter { $0.command != nil }
    }

    private var terminalLabel: String {
        L10n.string("New Terminal Tab (\(KeyBindingStore.shared.combo(for: .newTab).displayString))")
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing1) {
            AgentsFocusedLaunchButton(
                icon: .symbol("terminal"),
                label: terminalLabel,
                action: onOpenTerminal
            )
            ForEach(launchableOptions) { option in
                AgentsFocusedLaunchButton(
                    icon: .provider(option.provider.iconName),
                    label: L10n.string("New \(option.provider.displayName) Tab")
                ) {
                    launch(option)
                }
            }
        }
        .task(id: optionsLoadID) {
            await loadOptions(id: optionsLoadID)
        }
    }

    private func loadOptions(id: OptionsLoadID) async {
        loadedOptions = nil
        do {
            let options = try await AgentTabLaunchOptionsLoader.resolve(context: id.workspaceContext)
            loadedOptions = LoadedOptions(id: id, options: options)
        } catch is CancellationError {
            return
        } catch {
            guard !Task.isCancelled else { return }
            ToastState.shared.show(
                title: L10n.string("Could not check remote agent providers"),
                body: error.localizedDescription
            )
        }
    }

    private func launch(_ option: AgentTabLaunchOption) {
        guard let command = option.command else { return }
        AgentsFocusedTabLauncher.launch(
            request: AgentsFocusedTabLaunchRequest(
                project: project,
                worktree: worktree,
                providerID: option.provider.id,
                name: option.provider.displayName,
                command: command
            ),
            appState: appState,
            worktreeStore: worktreeStore
        )
    }
}

private struct AgentsFocusedLaunchButton: View {
    enum Icon {
        case symbol(String)
        case provider(String)
    }

    let icon: Icon
    let label: String
    let action: () -> Void

    @State private var hovered = false

    private var tileSize: CGFloat { UIMetrics.scaled(40) }

    var body: some View {
        Button(action: action) {
            iconView
                .frame(width: tileSize, height: tileSize)
                .background {
                    RoundedRectangle(cornerRadius: UIMetrics.radiusLG, style: .continuous)
                        .fill(hovered ? MuxyTheme.hover : Color.clear)
                }
                .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusLG, style: .continuous))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(label)
        .accessibilityLabel(label)
    }

    @ViewBuilder
    private var iconView: some View {
        switch icon {
        case let .symbol(name):
            Image(systemName: name)
                .font(.system(size: UIMetrics.iconLG, weight: .regular))
                .foregroundStyle(foreground)
        case let .provider(name):
            ProviderIconView(
                iconName: name,
                size: UIMetrics.iconLG,
                monochromeTint: foreground,
                forceMonochrome: true
            )
        }
    }

    private var foreground: Color {
        hovered ? MuxyTheme.fg : MuxyTheme.fgMuted
    }
}
