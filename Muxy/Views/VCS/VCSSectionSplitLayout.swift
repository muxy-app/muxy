import SwiftUI

struct VCSSectionSplitLayout: View {
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
                    LazyVStack(spacing: 0) {
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
                        LazyVStack(spacing: 0) {
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
            diffModeToggle
            expandCollapseButton(for: state.stagedFiles)
            IconButton(symbol: "minus", size: 11) {
                state.unstageAll()
            }
            .help("Unstage all")

        case .changes:
            diffModeToggle
            expandCollapseButton(for: state.unstagedFiles)
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

    private var diffModeToggle: some View {
        Button {
            state.mode = state.mode == .unified ? .split : .unified
        } label: {
            Image(systemName: state.mode == .unified ? "rectangle.split.2x1" : "rectangle")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 18, height: 18)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(state.mode == .unified ? "Switch to Split View" : "Switch to Unified View")
    }

    @ViewBuilder
    private func expandCollapseButton(for files: [GitStatusFile]) -> some View {
        let anyExpanded = files.contains { state.expandedFilePaths.contains($0.path) }
        Button {
            state.setExpanded(files: files, expanded: !anyExpanded)
        } label: {
            Image(systemName: anyExpanded ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: 18, height: 18)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(anyExpanded ? "Collapse all" : "Expand all")
    }

    private func fileSection(_ file: GitStatusFile, isStaged: Bool) -> some View {
        let expanded = state.expandedFilePaths.contains(file.path)
        let stats = state.displayedStats(for: file)
        let statusText = isStaged ? file.stagedStatusText : file.unstagedStatusText

        return VStack(spacing: 0) {
            VCSFileRow(
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
                .background(MuxyTheme.bg)
        } else if let error = state.diffErrorsByPath[file.path] {
            Text(error)
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
                .background(MuxyTheme.bg)
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
            .background(MuxyTheme.bg)
        } else {
            Text("No diff output")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(12)
                .background(MuxyTheme.bg)
        }
    }
}

enum SectionKind: Hashable {
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

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

struct VCSFileRow: View {
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
