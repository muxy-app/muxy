import SwiftUI

struct VCSTabView: View {
    @Bindable var state: VCSTabState
    let focused: Bool
    let onFocus: () -> Void

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
    }

    private var header: some View {
        HStack(spacing: 0) {
            if let branch = state.branchName {
                HStack(spacing: 6) {
                    Image(systemName: "arrow.triangle.branch")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)

                    Text(branch)
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fg)
                        .lineLimit(1)
                        .truncationMode(.middle)

                    if let prInfo = state.pullRequestInfo {
                        PRBadge(info: prInfo)
                    }
                }
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
        } else if state.files.isEmpty {
            Text(state.errorMessage ?? "No changes")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(state.files) { file in
                        fileSection(file)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func fileSection(_ file: GitStatusFile) -> some View {
        let expanded = state.expandedFilePaths.contains(file.path)
        let stats = state.displayedStats(for: file)

        return VStack(spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: expanded ? "chevron.down" : "chevron.right")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .frame(width: 12)

                FileDiffIcon()
                    .stroke(MuxyTheme.accent, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                    .frame(width: 11, height: 11)

                Text(file.path)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .frame(maxWidth: .infinity, alignment: .leading)

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
            .onTapGesture {
                onFocus()
                state.toggleExpanded(filePath: file.path)
            }

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
