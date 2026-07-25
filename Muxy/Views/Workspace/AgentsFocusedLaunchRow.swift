import SwiftUI

struct AgentsFocusedLaunchRow: View {
    let project: Project
    let worktree: Worktree
    let onOpenTerminal: () -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore
    @Environment(ProjectGroupStore.self) private var projectGroupStore
    @State private var options: [AgentTabLaunchOption] = []

    private var workspaceContext: WorkspaceContext {
        projectGroupStore.workspaceContext(for: project)
    }

    private var launchableOptions: [AgentTabLaunchOption] {
        options.filter { $0.command != nil }
    }

    private var terminalLabel: String {
        "New Terminal Tab (\(KeyBindingStore.shared.combo(for: .newTab).displayString))"
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
                    label: "New \(option.provider.displayName) Tab"
                ) {
                    launch(option)
                }
            }
        }
        .task(id: project.id) { await loadOptions() }
    }

    private func loadOptions() async {
        guard case let .ssh(destination) = workspaceContext else {
            options = AgentTabLaunchOption.resolveLocal()
            return
        }
        options = await (try? AgentTabLaunchOption.resolveRemote(destination: destination)) ?? []
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
