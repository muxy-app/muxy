import AppKit
import MuxyShared
import SwiftUI

struct ProjectRow: View {
    let project: Project
    let shortcutIndex: Int?
    let isAnyDragging: Bool
    let onSelect: () -> Void
    let onRemove: () -> Void
    let onRename: (String) -> Void
    let onSetLogo: (String?) -> Void
    let onSetIconColor: (String?) -> Void

    @Environment(AppState.self) private var appState
    @Environment(WorktreeStore.self) private var worktreeStore

    @State private var hovered = false
    @State private var isRenaming = false
    @State private var renameText = ""
    @State private var showWorktreePopover = false
    @State private var isGitRepo = false
    @State private var showCreateWorktreeSheet = false
    @State private var logoCropImage: IdentifiableImage?
    @State private var isRefreshingWorktrees = false
    @State private var showColorPicker = false

    private var isActive: Bool {
        appState.activeProjectID == project.id
    }

    private var worktrees: [Worktree] {
        worktreeStore.list(for: project.id)
    }

    private var displayLetter: String {
        String(project.name.prefix(1)).uppercased()
    }

    var body: some View {
        projectIcon
            .help(project.name)
            .contentShape(RoundedRectangle(cornerRadius: 8))
            .accessibilityElement(children: .combine)
            .accessibilityLabel(project.name)
            .accessibilityValue(isActive ? "Active" : "")
            .accessibilityAddTraits(isActive ? .isSelected : [])
            .accessibilityAddTraits(.isButton)
            .onHover { hovering in
                guard !isAnyDragging else { return }
                hovered = hovering
            }
            .onChange(of: isAnyDragging) { _, dragging in
                if dragging { hovered = false }
            }
            .onTapGesture {
                guard !isAnyDragging else { return }
                onSelect()
            }
            .task(id: project.path) {
                isGitRepo = await GitWorktreeService.shared.isGitRepository(project.path)
            }
            .contextMenu {
                Button("Set Logo...") { pickLogoImage() }
                if project.logo != nil {
                    Button("Remove Logo") { onSetLogo(nil) }
                }
                Button("Set Icon Color...") { showColorPicker = true }
                if project.iconColor != nil {
                    Button("Reset Icon Color") { onSetIconColor(nil) }
                }
                Divider()
                Button("Rename Project") { startRename() }
                if isGitRepo {
                    Divider()
                    Button("Refresh Worktrees") { Task { await refreshWorktrees() } }
                    Button("New Worktree…") { showCreateWorktreeSheet = true }
                    if worktrees.count > 1 {
                        Button("Switch Worktree…") { showWorktreePopover = true }
                    }
                }
                Divider()
                Button("Remove Project", role: .destructive, action: onRemove)
            }
            .popover(isPresented: $showWorktreePopover, arrowEdge: .trailing) {
                WorktreePopover(
                    project: project,
                    isGitRepo: isGitRepo,
                    onDismiss: { showWorktreePopover = false },
                    onRequestCreate: {
                        showWorktreePopover = false
                        showCreateWorktreeSheet = true
                    }
                )
                .environment(appState)
                .environment(worktreeStore)
            }
            .sheet(isPresented: $showCreateWorktreeSheet) {
                CreateWorktreeSheet(project: project) { result in
                    showCreateWorktreeSheet = false
                    handleCreateWorktreeResult(result)
                }
            }
            .sheet(item: $logoCropImage) { item in
                LogoCropperSheet(
                    sourceImage: item.image,
                    onConfirm: { cropped in
                        logoCropImage = nil
                        let logoPath = ProjectLogoStorage.save(
                            croppedImage: cropped,
                            forProjectID: project.id
                        )
                        onSetLogo(logoPath)
                    },
                    onCancel: { logoCropImage = nil }
                )
            }
            .overlay {
                if showShortcutBadge, let shortcutIndex,
                   let action = ShortcutAction.projectAction(for: shortcutIndex)
                {
                    ShortcutBadge(label: KeyBindingStore.shared.combo(for: action).displayString)
                }
            }
            .popover(isPresented: $isRenaming, arrowEdge: .trailing) {
                RenamePopover(
                    text: $renameText,
                    onCommit: { commitRename() },
                    onCancel: { cancelRename() }
                )
            }
            .popover(isPresented: $showColorPicker, arrowEdge: .trailing) {
                ProjectIconColorPicker(selectedID: project.iconColor) { id in
                    onSetIconColor(id)
                    showColorPicker = false
                }
            }
    }

    private var resolvedLogo: NSImage? {
        guard let filename = project.logo else { return nil }
        return NSImage(contentsOfFile: ProjectLogoStorage.logoPath(for: filename))
    }

    private var projectIcon: some View {
        let logo = resolvedLogo
        let unread = NotificationStore.shared.unreadCount(for: project.id)
        return ZStack {
            RoundedRectangle(cornerRadius: 6)
                .fill(iconBackground(hasLogo: logo != nil))

            if let logo {
                Image(nsImage: logo)
                    .resizable()
                    .scaledToFill()
                    .frame(width: 28, height: 28)
                    .clipShape(RoundedRectangle(cornerRadius: 6))
            } else {
                Text(displayLetter)
                    .font(.system(size: 13, weight: .bold))
                    .foregroundStyle(letterForeground)
            }
        }
        .frame(width: 28, height: 28)
        .padding(3)
        .overlay(alignment: .topTrailing) {
            if unread > 0 {
                NotificationBadge(count: unread)
                    .offset(x: 4, y: -4)
            }
        }
        .overlay {
            RoundedRectangle(cornerRadius: 11)
                .strokeBorder(isActive ? MuxyTheme.accent : .clear, lineWidth: 1.5)
                .animation(.easeInOut(duration: 0.15), value: isActive)
        }
        .overlay(alignment: .bottomTrailing) {
            if isRefreshingWorktrees {
                ProgressView()
                    .controlSize(.mini)
                    .padding(4)
            }
        }
    }

    private func iconBackground(hasLogo: Bool) -> AnyShapeStyle {
        if hasLogo { return AnyShapeStyle(Color.clear) }
        if let tint = ProjectIconColor.color(for: project.iconColor) {
            return AnyShapeStyle(hovered ? tint.opacity(0.85) : tint)
        }
        if hovered { return AnyShapeStyle(MuxyTheme.fg.opacity(0.22)) }
        return AnyShapeStyle(MuxyTheme.fg.opacity(0.18))
    }

    private var letterForeground: Color {
        if let foreground = ProjectIconColor.foreground(for: project.iconColor) {
            return foreground
        }
        return isActive ? MuxyTheme.fg : MuxyTheme.fgMuted
    }

    private var showShortcutBadge: Bool {
        guard let shortcutIndex,
              let action = ShortcutAction.projectAction(for: shortcutIndex)
        else { return false }
        return ModifierKeyMonitor.shared.isHolding(
            modifiers: KeyBindingStore.shared.combo(for: action).modifiers
        )
    }

    private func pickLogoImage() {
        let panel = NSOpenPanel()
        panel.title = "Choose a Logo Image"
        panel.allowedContentTypes = [.image]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false

        guard panel.runModal() == .OK,
              let url = panel.url,
              let image = NSImage(contentsOf: url)
        else { return }

        logoCropImage = IdentifiableImage(image: image)
    }

    private func handleCreateWorktreeResult(_ result: CreateWorktreeResult) {
        switch result {
        case let .created(worktree, runSetup):
            appState.selectWorktree(projectID: project.id, worktree: worktree)
            if runSetup,
               let paneID = appState.focusedArea(for: project.id)?.activeTab?.content.pane?.id
            {
                Task {
                    await WorktreeSetupRunner.run(
                        sourceProjectPath: project.path,
                        paneID: paneID
                    )
                }
            }
        case .cancelled:
            break
        }
    }

    private func startRename() {
        renameText = project.name
        isRenaming = true
    }

    private func commitRename() {
        let trimmed = renameText.trimmingCharacters(in: .whitespaces)
        if !trimmed.isEmpty {
            onRename(trimmed)
        }
        isRenaming = false
    }

    private func cancelRename() {
        isRenaming = false
    }

    private func refreshWorktrees() async {
        await WorktreeRefreshHelper.refresh(
            project: project,
            appState: appState,
            worktreeStore: worktreeStore,
            isRefreshing: $isRefreshingWorktrees
        )
    }
}

private struct RenamePopover: View {
    @Binding var text: String
    let onCommit: () -> Void
    let onCancel: () -> Void
    @FocusState private var isFocused: Bool

    var body: some View {
        VStack(spacing: 8) {
            Text("Rename Project")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            TextField("Project name", text: $text)
                .textFieldStyle(.roundedBorder)
                .font(.system(size: 12))
                .focused($isFocused)
                .onSubmit { onCommit() }
                .onExitCommand { onCancel() }
        }
        .padding(12)
        .frame(width: 200)
        .onAppear { isFocused = true }
    }
}

private struct IdentifiableImage: Identifiable {
    let id = UUID()
    let image: NSImage
}
