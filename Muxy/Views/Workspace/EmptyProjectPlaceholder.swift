import SwiftUI

struct EmptyProjectPlaceholder: View {
    let project: Project
    let worktree: Worktree
    let layout: AppLayout
    let onCreateTab: () -> Void

    private var showsAgentLaunchers: Bool { layout == .agentsFocused }

    private var subtitle: LocalizedStringResource {
        showsAgentLaunchers
            ? "Start an agent or open a terminal tab in this project."
            : "Open a new terminal tab to start working in this project."
    }

    var body: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Spacer()
            Image(systemName: "macwindow.badge.plus")
                .font(.system(size: UIMetrics.fontMega))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text(L10n.resource("No tabs in \(project.localizedDisplayName)"))
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text(L10n.resource(subtitle))
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: UIMetrics.scaled(360))
            actions
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private var actions: some View {
        if showsAgentLaunchers {
            AgentsFocusedLaunchRow(
                project: project,
                worktree: worktree,
                onOpenTerminal: onCreateTab
            )
        } else {
            newTabButton
        }
    }

    private var newTabButton: some View {
        Button(action: onCreateTab) {
            HStack(spacing: UIMetrics.spacing4) {
                Text(L10n.resource("New Tab"))
                Text(KeyBindingStore.shared.combo(for: .newTab).displayString)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .rounded))
                    .opacity(0.72)
            }
        }
        .buttonStyle(.borderedProminent)
    }
}
