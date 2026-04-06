import SwiftUI

struct VCSTabView: View {
    @Bindable var state: VCSTabState
    let focused: Bool
    let onFocus: () -> Void
    @State private var showDiscardAllConfirmation = false
    @State private var pendingDiscardPath: String?

    private var commitEnabled: Bool {
        state.hasStagedChanges && !state.commitMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            content
        }
        .background(MuxyTheme.terminalBg)
        .contentShape(Rectangle())
        .onTapGesture(perform: onFocus)
        .onAppear {
            if state.files.isEmpty, !state.isLoadingFiles {
                state.refresh()
            }
        }
        .alert("Discard All Changes?", isPresented: $showDiscardAllConfirmation) {
            Button("Discard All", role: .destructive) { state.discardAll() }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This will discard all uncommitted changes. This cannot be undone.")
        }
        .alert(
            "Discard Changes?",
            isPresented: Binding(
                get: { pendingDiscardPath != nil },
                set: { if !$0 { pendingDiscardPath = nil } }
            )
        ) {
            Button("Discard", role: .destructive) {
                guard let path = pendingDiscardPath else { return }
                state.discardFile(path)
                pendingDiscardPath = nil
            }
            Button("Cancel", role: .cancel) { pendingDiscardPath = nil }
        } message: {
            if let path = pendingDiscardPath {
                Text("Discard changes to \((path as NSString).lastPathComponent)?")
            }
        }
        .alert(
            "Error",
            isPresented: Binding(
                get: { state.statusIsError && state.statusMessage != nil },
                set: { if !$0 { state.statusMessage = nil } }
            )
        ) {
            Button("OK", role: .cancel) { state.statusMessage = nil }
        } message: {
            if let message = state.statusMessage {
                Text(message)
            }
        }
    }

    private var header: some View {
        HStack(spacing: 0) {
            BranchPicker(
                currentBranch: state.branchName,
                branches: state.branches,
                isLoading: state.isLoadingBranches,
                onSelect: { state.switchBranch($0) },
                onRefresh: { state.loadBranches() }
            )

            if let prInfo = state.pullRequestInfo {
                PRBadge(info: prInfo)
                    .padding(.leading, 6)
            }

            Spacer(minLength: 0)

            HStack(spacing: 0) {
                ForEach(VCSTabState.ViewMode.allCases) { mode in
                    Button {
                        state.mode = mode
                    } label: {
                        Text(mode.title)
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(state.mode == mode ? MuxyTheme.fg : MuxyTheme.fgDim)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background(
                                RoundedRectangle(cornerRadius: 6)
                                    .fill(state.mode == mode ? MuxyTheme.surface : .clear)
                            )
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.trailing, 6)

            Button {
                if state.expandedFilePaths.isEmpty {
                    state.expandAll()
                } else {
                    state.collapseAll()
                }
            } label: {
                Text(state.expandedFilePaths.isEmpty ? "Expand all" : "Collapse all")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(
                        RoundedRectangle(cornerRadius: 6)
                            .fill(MuxyTheme.surface)
                    )
            }
            .buttonStyle(.plain)
            .padding(.trailing, 6)

            Menu {
                Toggle("Hide Whitespace Changes", isOn: Binding(
                    get: { state.hideWhitespace },
                    set: { _ in state.toggleWhitespace() }
                ))
            } label: {
                Image(systemName: "gearshape")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: 24, height: 24)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .frame(width: 24)

            IconButton(symbol: "arrow.clockwise") {
                state.refresh()
            }
        }
        .padding(.trailing, 4)
        .padding(.leading, 8)
        .frame(height: 32)
        .background(MuxyTheme.bg)
    }

    @ViewBuilder
    private var content: some View {
        if state.isLoadingFiles {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if state.files.isEmpty, state.errorMessage != nil {
            Text(state.errorMessage ?? "")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollView {
                VStack(spacing: 0) {
                    commitArea
                    Rectangle().fill(MuxyTheme.border).frame(height: 1)

                    if state.files.isEmpty {
                        Text("No changes")
                            .font(.system(size: 12))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 40)
                    } else {
                        if !state.stagedFiles.isEmpty {
                            stagedSection
                        }
                        changesSection
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private var commitArea: some View {
        VStack(spacing: 8) {
            ZStack(alignment: .topLeading) {
                if state.commitMessage.isEmpty {
                    Text("Commit message (Enter to commit on \(state.branchName ?? "branch"))")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 10)
                        .allowsHitTesting(false)
                }
                TextEditor(text: $state.commitMessage)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fg)
                    .scrollContentBackground(.hidden)
                    .padding(.horizontal, 5)
                    .padding(.vertical, 9)
                    .frame(minHeight: 54, maxHeight: 100)
                    .onKeyPress(.return, phases: .down) { keyPress in
                        if keyPress.modifiers.contains(.shift) {
                            return .ignored
                        }
                        state.commit()
                        return .handled
                    }
            }
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(MuxyTheme.border, lineWidth: 1))

            HStack(spacing: 6) {
                Button {
                    state.commit()
                } label: {
                    HStack(spacing: 4) {
                        if state.isCommitting {
                            ProgressView()
                                .controlSize(.mini)
                        } else {
                            Image(systemName: "checkmark")
                                .font(.system(size: 10, weight: .bold))
                        }
                        Text("Commit")
                            .font(.system(size: 11, weight: .medium))
                    }
                    .foregroundStyle(commitEnabled ? MuxyTheme.bg : MuxyTheme.fgDim)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 6)
                    .background(
                        commitEnabled ? MuxyTheme.accent : MuxyTheme.surface,
                        in: RoundedRectangle(cornerRadius: 6)
                    )
                }
                .buttonStyle(.plain)
                .disabled(!commitEnabled || state.isCommitting)

                Button {
                    state.push()
                } label: {
                    HStack(spacing: 4) {
                        if state.isPushing {
                            ProgressView()
                                .controlSize(.mini)
                        } else {
                            Image(systemName: "arrow.up")
                                .font(.system(size: 10, weight: .bold))
                        }
                        Text("Push")
                            .font(.system(size: 11, weight: .medium))
                    }
                    .foregroundStyle(MuxyTheme.fg)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain)
                .disabled(state.isPushing)

                Button {
                    state.pull()
                } label: {
                    HStack(spacing: 4) {
                        if state.isPulling {
                            ProgressView()
                                .controlSize(.mini)
                        } else {
                            Image(systemName: "arrow.down")
                                .font(.system(size: 10, weight: .bold))
                        }
                        Text("Pull")
                            .font(.system(size: 11, weight: .medium))
                    }
                    .foregroundStyle(MuxyTheme.fg)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain)
                .disabled(state.isPulling)
            }
        }
        .padding(10)
        .background(MuxyTheme.bg)
    }

    private var stagedSection: some View {
        VStack(spacing: 0) {
            sectionHeader(
                title: "Staged Changes",
                count: state.stagedFiles.count,
                actions: {
                    IconButton(symbol: "minus", size: 11) {
                        state.unstageAll()
                    }
                    .help("Unstage all")
                }
            )

            ForEach(state.stagedFiles) { file in
                fileSection(file, isStaged: true)
            }
        }
    }

    private var changesSection: some View {
        VStack(spacing: 0) {
            sectionHeader(
                title: "Changes",
                count: state.unstagedFiles.count,
                actions: {
                    IconButton(symbol: "plus", size: 11) {
                        state.stageAll()
                    }
                    .help("Stage all")

                    IconButton(symbol: "arrow.uturn.backward", size: 11) {
                        showDiscardAllConfirmation = true
                    }
                    .help("Discard all changes")
                }
            )

            ForEach(state.unstagedFiles) { file in
                fileSection(file, isStaged: false)
            }
        }
    }

    private func sectionHeader(
        title: String,
        count: Int,
        @ViewBuilder actions: () -> some View
    ) -> some View {
        HStack(spacing: 6) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)

            Text("\(count)")
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(MuxyTheme.bg)
                .padding(.horizontal, 6)
                .padding(.vertical, 1)
                .background(MuxyTheme.fgMuted, in: Capsule())

            Spacer(minLength: 0)

            actions()
        }
        .padding(.horizontal, 10)
        .frame(height: 30)
        .background(MuxyTheme.bg)
    }

    private func fileSection(_ file: GitStatusFile, isStaged: Bool) -> some View {
        let expanded = state.expandedFilePaths.contains(file.path)
        let stats = state.displayedStats(for: file)
        let statusText = isStaged ? file.stagedStatusText : file.unstagedStatusText

        return VStack(spacing: 0) {
            FileRow(
                file: file,
                statusText: statusText,
                expanded: expanded,
                stats: stats,
                isStaged: isStaged,
                onToggle: {
                    onFocus()
                    state.toggleExpanded(filePath: file.path)
                },
                onStage: { state.stageFile(file.path) },
                onUnstage: { state.unstageFile(file.path) },
                onDiscard: { pendingDiscardPath = file.path }
            )

            if expanded {
                expandedDiff(for: file)
            }

            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func expandedDiff(for file: GitStatusFile) -> some View {
        if state.loadingDiffPaths.contains(file.path) {
            ProgressView()
                .frame(maxWidth: .infinity)
                .padding(14)
                .background(MuxyTheme.terminalBg)
        } else if let error = state.diffErrorsByPath[file.path] {
            Text(error)
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
                .background(MuxyTheme.terminalBg)
        } else if let diff = state.diffsByPath[file.path] {
            VStack(spacing: 0) {
                if diff.truncated {
                    HStack {
                        Text("Large diff preview")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(MuxyTheme.fgMuted)
                        Spacer(minLength: 0)
                        Button("Load full diff") {
                            state.loadFullDiff(filePath: file.path)
                        }
                        .buttonStyle(.plain)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(MuxyTheme.accent)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(MuxyTheme.bg)
                    Rectangle().fill(MuxyTheme.border).frame(height: 1)
                }

                switch state.mode {
                case .unified:
                    UnifiedDiffView(rows: diff.rows, filePath: file.path)
                case .split:
                    SplitDiffView(rows: diff.rows, filePath: file.path)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(MuxyTheme.terminalBg)
        } else {
            Text("No diff output")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
                .background(MuxyTheme.terminalBg)
        }
    }
}

private struct FileRow: View {
    let file: GitStatusFile
    let statusText: String
    let expanded: Bool
    let stats: VCSTabState.FileStats
    let isStaged: Bool
    let onToggle: () -> Void
    let onStage: () -> Void
    let onUnstage: () -> Void
    let onDiscard: () -> Void
    @State private var hovered = false

    private var statusColor: Color {
        switch statusText.first {
        case "A":
            MuxyTheme.diffAddFg
        case "D":
            MuxyTheme.diffRemoveFg
        case "M":
            MuxyTheme.accent
        case "R":
            MuxyTheme.accent
        case "U":
            MuxyTheme.diffAddFg
        default:
            MuxyTheme.fgMuted
        }
    }

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: expanded ? "chevron.down" : "chevron.right")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(width: 12)

            Text(statusText)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .foregroundStyle(statusColor)
                .frame(width: 14)

            FileDiffIcon()
                .stroke(statusColor, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                .frame(width: 11, height: 11)

            Text(file.path)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(maxWidth: .infinity, alignment: .leading)

            if hovered {
                actionButtons
            }

            if stats.binary {
                Text("Binary")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgMuted)
            } else {
                if let additions = stats.additions {
                    Text("+\(additions)")
                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.diffAddFg)
                }
                if let deletions = stats.deletions {
                    Text("-\(deletions)")
                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.diffRemoveFg)
                }
            }
        }
        .padding(.horizontal, 10)
        .frame(height: 34)
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .onTapGesture(perform: onToggle)
    }

    private var actionButtons: some View {
        HStack(spacing: 0) {
            if isStaged {
                IconButton(symbol: "minus", size: 11, action: onUnstage)
                    .help("Unstage")
            } else {
                IconButton(symbol: "plus", size: 11, action: onStage)
                    .help("Stage")
                IconButton(symbol: "arrow.uturn.backward", size: 11, action: onDiscard)
                    .help("Discard changes")
            }
        }
    }
}
