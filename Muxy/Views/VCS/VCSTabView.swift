import AppKit
import SwiftUI

struct VCSTabView: View {
    @Bindable var state: VCSTabState
    let focused: Bool
    let onFocus: () -> Void
    @Environment(AppState.self) private var appState
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
            if !state.hasCompletedInitialLoad, !state.isLoadingFiles {
                state.refresh()
            }
        }
        .onChange(of: state.projectPath) {
            if !state.hasCompletedInitialLoad, !state.isLoadingFiles {
                state.refresh()
            }
        }
        .onChange(of: state.showPushUpstreamConfirmation) { _, show in
            guard show else { return }
            state.showPushUpstreamConfirmation = false
            presentPushUpstreamConfirmation()
        }
        .onChange(of: showDiscardAllConfirmation) { _, show in
            guard show else { return }
            showDiscardAllConfirmation = false
            presentDiscardConfirmation(
                title: "Discard All Changes?",
                message: "This will discard all uncommitted changes. This cannot be undone.",
                buttonTitle: "Discard All"
            ) {
                state.discardAll()
            }
        }
        .onChange(of: pendingDiscardPath) { _, path in
            guard let path else { return }
            pendingDiscardPath = nil
            let fileName = (path as NSString).lastPathComponent
            presentDiscardConfirmation(
                title: "Discard Changes?",
                message: "Discard changes to \(fileName)?",
                buttonTitle: "Discard"
            ) {
                state.discardFile(path)
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
            VStack(spacing: 0) {
                commitArea
                SectionSplitLayout(
                    state: state,
                    onFocus: onFocus,
                    showDiscardAllConfirmation: $showDiscardAllConfirmation,
                    pendingDiscardPath: $pendingDiscardPath,
                    onOpenInEditor: openFileInEditor
                )
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

    private func presentDiscardConfirmation(
        title: String,
        message: String,
        buttonTitle: String,
        onConfirm: @escaping () -> Void
    ) {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: buttonTitle)
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = "\r"
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            if response == .alertFirstButtonReturn {
                onConfirm()
            }
        }
    }

    private func presentPushUpstreamConfirmation() {
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              window.attachedSheet == nil
        else { return }

        let branch = state.branchName ?? "current branch"
        let alert = NSAlert()
        alert.messageText = "Push to Remote?"
        alert.informativeText = "The branch \"\(branch)\" has no upstream on the remote. Push and set upstream to origin/\(branch)?"
        alert.alertStyle = .informational
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: "Push")
        alert.addButton(withTitle: "Cancel")
        alert.buttons.first?.keyEquivalent = "\r"
        alert.buttons.last?.keyEquivalent = "\u{1b}"

        alert.beginSheetModal(for: window) { response in
            if response == .alertFirstButtonReturn {
                state.pushSetUpstream()
            }
        }
    }

    private func openFileInEditor(_ relativePath: String) {
        guard let projectID = appState.activeProjectID else { return }
        let fullPath = state.projectPath.hasSuffix("/")
            ? state.projectPath + relativePath
            : state.projectPath + "/" + relativePath
        appState.openFile(fullPath, projectID: projectID)
    }
}

private struct SectionSplitLayout: View {
    @Bindable var state: VCSTabState
    let onFocus: () -> Void
    @Binding var showDiscardAllConfirmation: Bool
    @Binding var pendingDiscardPath: String?
    let onOpenInEditor: (String) -> Void

    private static let sectionHeaderHeight: CGFloat = 30

    private var hasStaged: Bool { !state.stagedFiles.isEmpty }

    private var sections: [SectionKind] {
        var result: [SectionKind] = []
        if hasStaged { result.append(.staged) }
        result.append(.changes)
        result.append(.history)
        return result
    }

    private func isCollapsed(_ kind: SectionKind) -> Bool {
        switch kind {
        case .staged: state.stagedCollapsed
        case .changes: state.changesCollapsed
        case .history: state.historyCollapsed
        }
    }

    private func toggleCollapsed(_ kind: SectionKind) {
        switch kind {
        case .staged: state.stagedCollapsed.toggle()
        case .changes: state.changesCollapsed.toggle()
        case .history:
            state.historyCollapsed.toggle()
            if !state.historyCollapsed, state.commits.isEmpty {
                state.loadCommits()
            }
        }
    }

    var body: some View {
        GeometryReader { geo in
            let allSections = sections
            let expandedSections = allSections.filter { !isCollapsed($0) }
            let collapsedSections = allSections.filter { isCollapsed($0) }
            let collapsedHeight = CGFloat(collapsedSections.count) * Self.sectionHeaderHeight
            let borderCount = CGFloat(allSections.count + 1)
            let availableForExpanded = max(0, geo.size.height - collapsedHeight - borderCount)
            let ratios = distributedRatios(allSections: allSections, expandedSections: expandedSections)

            VStack(spacing: 0) {
                ForEach(Array(allSections.enumerated()), id: \.element) { index, section in
                    let collapsed = isCollapsed(section)
                    let prevExpanded = previousExpandedSection(before: index, in: allSections)
                    let needsDraggableDivider = !collapsed && prevExpanded != nil

                    if needsDraggableDivider, let prev = prevExpanded {
                        sectionDivider(
                            above: prev,
                            below: section,
                            totalHeight: availableForExpanded,
                            allSections: allSections
                        )
                    } else {
                        Rectangle().fill(MuxyTheme.border).frame(height: 1)
                    }

                    if collapsed {
                        sectionHeader(for: section, collapsed: true)
                            .frame(height: Self.sectionHeaderHeight)
                    } else {
                        let ratio = ratios[section] ?? 0
                        let sectionHeight = max(Self.sectionHeaderHeight, availableForExpanded * ratio)
                        sectionView(for: section, height: sectionHeight)
                    }
                }
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
            }
        }
    }

    private func distributedRatios(
        allSections: [SectionKind],
        expandedSections: [SectionKind]
    ) -> [SectionKind: CGFloat] {
        guard !expandedSections.isEmpty else { return [:] }

        let rawRatios: [CGFloat] = allSections.enumerated().compactMap { idx, section in
            guard !isCollapsed(section) else { return nil }
            return state.sectionRatios[safe: idx] ?? (1.0 / CGFloat(expandedSections.count))
        }

        let sum = rawRatios.reduce(0, +)
        guard sum > 0 else { return [:] }

        var result: [SectionKind: CGFloat] = [:]
        var rawIdx = 0
        for section in expandedSections {
            result[section] = rawRatios[rawIdx] / sum
            rawIdx += 1
        }
        return result
    }

    private func previousExpandedSection(before index: Int, in allSections: [SectionKind]) -> SectionKind? {
        for i in stride(from: index - 1, through: 0, by: -1) where !isCollapsed(allSections[i]) {
            return allSections[i]
        }
        return nil
    }

    private func sectionDivider(
        above: SectionKind,
        below: SectionKind,
        totalHeight: CGFloat,
        allSections: [SectionKind]
    ) -> some View {
        Rectangle().fill(MuxyTheme.border).frame(height: 1)
            .overlay {
                Color.clear
                    .frame(height: 5)
                    .contentShape(Rectangle())
                    .gesture(
                        DragGesture(minimumDistance: 1)
                            .onChanged { v in
                                guard totalHeight > 0 else { return }
                                let delta = v.translation.height / totalHeight

                                guard let aboveIdx = allSections.firstIndex(of: above),
                                      let belowIdx = allSections.firstIndex(of: below)
                                else { return }

                                var ratios = state.sectionRatios
                                let minRatio: CGFloat = 0.08

                                ratios[aboveIdx] += delta
                                ratios[belowIdx] -= delta

                                ratios[aboveIdx] = max(minRatio, ratios[aboveIdx])
                                ratios[belowIdx] = max(minRatio, ratios[belowIdx])

                                let sum = ratios.reduce(0, +)
                                if sum > 0 {
                                    ratios = ratios.map { $0 / sum }
                                }

                                state.sectionRatios = ratios
                            }
                    )
                    .onHover { on in
                        if on { NSCursor.resizeUpDown.push() } else { NSCursor.pop() }
                    }
            }
    }

    @ViewBuilder
    private func sectionView(for section: SectionKind, height: CGFloat) -> some View {
        switch section {
        case .staged:
            VStack(spacing: 0) {
                sectionHeader(for: .staged, collapsed: false)
                ScrollView {
                    VStack(spacing: 0) {
                        ForEach(state.stagedFiles) { file in
                            fileSection(file, isStaged: true)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .frame(height: height)

        case .changes:
            VStack(spacing: 0) {
                sectionHeader(for: .changes, collapsed: false)
                if state.files.isEmpty {
                    Text("No changes")
                        .font(.system(size: 12))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    ScrollView {
                        VStack(spacing: 0) {
                            ForEach(state.unstagedFiles) { file in
                                fileSection(file, isStaged: false)
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
            }
            .frame(height: height)

        case .history:
            VStack(spacing: 0) {
                sectionHeader(for: .history, collapsed: false)
                CommitHistoryView(state: state)
            }
            .frame(height: height)
        }
    }

    private func sectionHeader(for section: SectionKind, collapsed: Bool) -> some View {
        let isCollapsedState = collapsed

        return HStack(spacing: 6) {
            Button {
                toggleCollapsed(section)
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: isCollapsedState ? "chevron.right" : "chevron.down")
                        .font(.system(size: 9, weight: .bold))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .frame(width: 10)

                    Text(section.title)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                }
            }
            .buttonStyle(.plain)

            Text("\(sectionCount(for: section))")
                .font(.system(size: 10, weight: .bold))
                .foregroundStyle(MuxyTheme.bg)
                .padding(.horizontal, 6)
                .padding(.vertical, 1)
                .background(MuxyTheme.fgMuted, in: Capsule())

            Spacer(minLength: 0)

            sectionActions(for: section)
        }
        .padding(.horizontal, 10)
        .frame(height: Self.sectionHeaderHeight)
        .background(MuxyTheme.bg)
    }

    private func sectionCount(for section: SectionKind) -> Int {
        switch section {
        case .staged: state.stagedFiles.count
        case .changes: state.unstagedFiles.count
        case .history: state.commits.count
        }
    }

    @ViewBuilder
    private func sectionActions(for section: SectionKind) -> some View {
        switch section {
        case .staged:
            IconButton(symbol: "minus", size: 11) {
                state.unstageAll()
            }
            .help("Unstage all")

        case .changes:
            IconButton(symbol: "plus", size: 11) {
                state.stageAll()
            }
            .help("Stage all")

            IconButton(symbol: "arrow.uturn.backward", size: 11) {
                showDiscardAllConfirmation = true
            }
            .help("Discard all changes")

        case .history:
            IconButton(symbol: "arrow.clockwise", size: 11) {
                state.loadCommits()
            }
            .help("Refresh history")
        }
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
                onDiscard: { pendingDiscardPath = file.path },
                onOpenInEditor: { onOpenInEditor(file.path) }
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

private enum SectionKind: Hashable {
    case staged
    case changes
    case history

    var title: String {
        switch self {
        case .staged: "Staged Changes"
        case .changes: "Changes"
        case .history: "History"
        }
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
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
    let onOpenInEditor: () -> Void
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
            IconButton(symbol: "doc.text", size: 11, action: onOpenInEditor)
                .help("Open in Editor")
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
