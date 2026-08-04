import AppKit
import MuxySessionProtocol
import SwiftUI

struct TerminalSessionsButton: View {
    @Environment(AppState.self) private var appState
    @State private var store = TerminalSessionsStore.shared
    @State private var showingPopover = false
    @State private var hovered = false

    private var worktreeKey: WorktreeKey? {
        guard let projectID = appState.activeProjectID else { return nil }
        return appState.activeWorktreeKey(for: projectID)
    }

    private var sessions: [SessionDescriptor] {
        guard let worktreeKey else { return [] }
        return store.detachedSessions(
            inWorktree: worktreeKey,
            ownedSessionIDs: Set(appState.allTerminalPanes.map(\.sessionID))
        )
    }

    var body: some View {
        if sessions.isEmpty {
            refreshTrigger
        } else {
            StatusBarSeparator()
            button
        }
    }

    private var refreshTrigger: some View {
        Color.clear
            .frame(width: 0, height: 0)
            .onAppear { store.refresh() }
            .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
                store.refresh()
            }
    }

    private var button: some View {
        Button {
            showingPopover.toggle()
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "rectangle.stack")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                Text(verbatim: "\(sessions.count)")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                    .monospacedDigit()
            }
            .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
            .padding(.horizontal, 4)
            .frame(maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(L10n.string("Detached background terminals"))
        .accessibilityLabel(L10n.string("Detached background terminals"))
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            store.refresh()
        }
        .popover(isPresented: $showingPopover, arrowEdge: .bottom) {
            TerminalSessionsPopover(
                sessions: sessions,
                worktreeKey: worktreeKey,
                onClose: { showingPopover = false }
            )
            .onAppear { store.refresh() }
        }
    }
}

private struct TerminalSessionsPopover: View {
    let sessions: [SessionDescriptor]
    let worktreeKey: WorktreeKey?
    let onClose: () -> Void

    @Environment(AppState.self) private var appState
    @State private var store = TerminalSessionsStore.shared

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            header
            if sessions.isEmpty {
                Text(L10n.resource("Every background terminal in this worktree is open in a tab."))
                    .font(.system(size: UIMetrics.fontFootnote))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: UIMetrics.spacing4) {
                        ForEach(sessions, id: \.identifier.uuidString) { session in
                            row(session)
                        }
                    }
                }
                .frame(maxHeight: UIMetrics.scaled(320))
            }
        }
        .padding(UIMetrics.spacing6)
        .frame(width: UIMetrics.scaled(320))
        .background(MuxyTheme.bg)
    }

    private var header: some View {
        HStack {
            Text(L10n.resource("Detached terminals"))
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Spacer()
            Text(verbatim: "\(sessions.count)")
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .monospacedDigit()
        }
    }

    private func row(_ session: SessionDescriptor) -> some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing2) {
            Text(TerminalSessionsStore.displayTitle(for: session))
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            Text(session.workingDirectory)
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.head)
            HStack(spacing: UIMetrics.spacing3) {
                actionButton(L10n.resource("Active tab")) { attachToActiveTab(session) }
                actionButton(L10n.resource("New tab")) { attachInNewTab(session) }
                Spacer(minLength: 0)
                actionButton(L10n.resource("Stop"), isDestructive: true) { stop(session) }
            }
        }
        .padding(.vertical, UIMetrics.spacing2)
    }

    private func actionButton(
        _ label: LocalizedStringResource,
        isDestructive: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(label)
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                .foregroundStyle(isDestructive ? MuxyTheme.warning : MuxyTheme.accent)
        }
        .buttonStyle(.plain)
    }

    private func attachToActiveTab(_ session: SessionDescriptor) {
        guard let sessionID = UUID(uuidString: session.identifier.uuidString) else { return }
        _ = TerminalSessionAttachment.attachToActivePane(sessionID: sessionID, appState: appState)
        store.refresh()
        onClose()
    }

    private func attachInNewTab(_ session: SessionDescriptor) {
        guard let sessionID = UUID(uuidString: session.identifier.uuidString), let worktreeKey else { return }
        TerminalSessionAttachment.attachInNewTab(
            sessionID: sessionID,
            title: session.value(forMetadataKey: SessionMetadataKey.title),
            key: worktreeKey,
            appState: appState
        )
        store.refresh()
        onClose()
    }

    private func stop(_ session: SessionDescriptor) {
        guard let sessionID = UUID(uuidString: session.identifier.uuidString) else { return }
        PersistentSessionService.shared.endSession(sessionID: sessionID)
        store.refresh()
    }
}
