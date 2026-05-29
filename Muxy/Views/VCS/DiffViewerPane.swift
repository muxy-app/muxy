import SwiftUI

struct DiffViewerPane: View {
    @Bindable var state: DiffViewerTabState
    let tabArea: TabArea
    let focused: Bool
    let onFocus: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            DiffViewerSidebar(state: state)
                .frame(minWidth: UIMetrics.scaled(220), idealWidth: UIMetrics.scaled(280), maxWidth: UIMetrics.scaled(340))

            Rectangle().fill(MuxyTheme.border).frame(width: 1)

            VStack(spacing: 0) {
                DiffViewerBreadcrumb(state: state)
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
                selectedContent
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { onFocus() })
        .onAppear {
            state.tabArea = tabArea
            state.loadCommentAuthor()
            if state.source == .workingTree, !state.vcs.hasCompletedInitialLoad, !state.vcs.isLoadingFiles {
                state.vcs.refresh()
            }
            state.reconcileSelection()
            state.reconcileLargeDiffCollapse()
            state.loadAllDiffs()
        }
        .onChange(of: state.vcs.files) { _, _ in
            guard state.source == .workingTree else { return }
            state.reconcileSelection()
            state.reconcileLargeDiffCollapse()
            state.loadAllDiffs()
        }
        .onChange(of: state.vcs.diffCache.revision) { _, _ in
            guard state.source == .workingTree else { return }
            state.reconcileLargeDiffCollapse()
            state.reconcileCommentAnchors()
        }
        .onChange(of: state.diffCache.revision) { _, _ in
            guard state.source != .workingTree else { return }
            state.reconcileLargeDiffCollapse()
            state.reconcileCommentAnchors()
        }
    }

    @ViewBuilder
    private var selectedContent: some View {
        if !sections.isEmpty {
            VStack(spacing: 0) {
                if hasTruncatedDiff {
                    truncatedBanner
                    Rectangle().fill(MuxyTheme.border).frame(height: 1)
                }
                DiffCardList(state: state, sections: sections)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(fontShortcuts)
        } else if state.isLoadingFiles || isLoadingAnyDiff {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            VStack(spacing: UIMetrics.spacing5) {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(.system(size: UIMetrics.fontMega))
                    .foregroundStyle(MuxyTheme.fgDim)
                Text("No changed file selected")
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var sections: [DiffEditorFileSection] {
        sectionFiles.map { file, isStaged in
            let cacheKey = DiffViewerTabState.cacheKey(filePath: file.path, isStaged: isStaged)
            let diff = activeDiffCache.diff(for: cacheKey)
            return DiffEditorFileSection(
                filePath: file.path,
                cacheKey: cacheKey,
                rows: diff?.rows ?? [],
                isCollapsed: state.collapsedCacheKeys.contains(cacheKey),
                isLargeUnloaded: diff?.truncated == true && !state.manuallyLoadedCacheKeys.contains(cacheKey),
                isLoading: activeDiffCache.isLoading(cacheKey),
                errorMessage: activeDiffCache.error(for: cacheKey),
                additions: diff?.additions ?? file.additions(isStaged: isStaged) ?? 0,
                deletions: diff?.deletions ?? file.deletions(isStaged: isStaged) ?? 0,
                isStaged: isStaged
            )
        }
    }

    private var sectionFiles: [(GitStatusFile, Bool)] {
        state.stagedFiles.map { ($0, true) } + state.unstagedFiles.map { ($0, false) }
    }

    private var combinedCacheKey: String {
        sectionFiles.map { DiffViewerTabState.cacheKey(filePath: $0.0.path, isStaged: $0.1) }.joined(separator: "|")
            + ":\(state.mode.rawValue):\(sections.count):\(state.collapsedCacheKeys.sorted().joined(separator: ","))"
    }

    private var isLoadingAnyDiff: Bool {
        sectionFiles.contains { file, isStaged in
            activeDiffCache.isLoading(DiffViewerTabState.cacheKey(filePath: file.path, isStaged: isStaged))
        }
    }

    private var hasTruncatedDiff: Bool {
        sectionFiles.contains { file, isStaged in
            let cacheKey = DiffViewerTabState.cacheKey(filePath: file.path, isStaged: isStaged)
            return activeDiffCache.diff(for: cacheKey)?.truncated == true
        }
    }

    private var activeDiffCache: DiffCache {
        state.source == .workingTree ? state.vcs.diffCache : state.diffCache
    }

    private var truncatedBanner: some View {
        HStack {
            Text("Large diff preview")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
            Spacer(minLength: 0)
            Button("Load full diff") { state.refresh(forceFull: true) }
                .buttonStyle(.plain)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private var fontShortcuts: some View {
        Group {
            Button("Increase Diff Font Size") { state.adjustFontSize(by: 1) }
                .keyboardShortcut("=", modifiers: .command)
            Button("Decrease Diff Font Size") { state.adjustFontSize(by: -1) }
                .keyboardShortcut("-", modifiers: .command)
            Button("Reset Diff Font Size") { state.resetFontSize() }
                .keyboardShortcut("0", modifiers: .command)
        }
        .opacity(0)
        .frame(width: 0, height: 0)
        .accessibilityHidden(true)
    }
}

private struct DiffViewerBreadcrumb: View {
    @Bindable var state: DiffViewerTabState

    private var additions: Int {
        state.stagedFiles.compactMap { $0.additions(isStaged: true) }.reduce(0, +)
            + state.unstagedFiles.compactMap { $0.additions(isStaged: false) }.reduce(0, +)
    }

    private var deletions: Int {
        state.stagedFiles.compactMap { $0.deletions(isStaged: true) }.reduce(0, +)
            + state.unstagedFiles.compactMap { $0.deletions(isStaged: false) }.reduce(0, +)
    }

    private var diffFileCount: Int {
        state.stagedFiles.count + state.unstagedFiles.count
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            FileDiffIcon()
                .stroke(MuxyTheme.fgDim, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                .frame(width: UIMetrics.scaled(11), height: UIMetrics.scaled(11))

            Text(state.displayTitle)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)

            if let sourceLink = state.source.link {
                Link(destination: sourceLink.url) {
                    HStack(spacing: UIMetrics.scaled(3)) {
                        Image(systemName: "arrow.up.right.square")
                            .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                        Text(sourceLink.title)
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    }
                    .foregroundStyle(MuxyTheme.accent)
                    .padding(.horizontal, UIMetrics.scaled(6))
                    .frame(height: UIMetrics.controlSmall)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                    .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .help("Open \(sourceLink.title)")
            }

            Text("\(diffFileCount) files")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.scaled(5))
                .padding(.vertical, UIMetrics.scaled(1))
                .background(MuxyTheme.surface, in: Capsule())

            if additions > 0 {
                Text("+\(additions)")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                    .foregroundStyle(MuxyTheme.diffAddFg)
            }

            if deletions > 0 {
                Text("-\(deletions)")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            }

            Spacer()

            if !state.isPR, state.hasUnsentSessionComments {
                Menu {
                    ForEach(AIAssistantProvider.allCases) { provider in
                        Button(provider.displayName) { state.runAllByAgent(provider: provider) }
                    }
                } label: {
                    HStack(spacing: UIMetrics.scaled(4)) {
                        Image(systemName: "terminal")
                            .font(.system(size: UIMetrics.fontXS, weight: .semibold))
                        Text("Run all by agent")
                            .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    }
                    .foregroundStyle(MuxyTheme.accent)
                    .padding(.horizontal, UIMetrics.scaled(6))
                    .frame(height: UIMetrics.controlSmall)
                    .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                    .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
                .help("Run all comments by AI agent in a new tab")
            }

            collapseToggle

            wrapToggle

            modeToggle

            IconButton(symbol: "arrow.clockwise", size: 11, accessibilityLabel: "Refresh Diff") {
                state.refresh(forceFull: false)
            }
            .help("Refresh")
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .frame(height: UIMetrics.scaled(32))
        .background(MuxyTheme.bg)
    }

    private var wrapToggle: some View {
        Button {
            state.wordWrap.toggle()
        } label: {
            Text("Wrap")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(state.wordWrap ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.spacing3)
                .frame(height: UIMetrics.controlSmall)
                .background(state.wordWrap ? MuxyTheme.surface : Color.clear, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(state.wordWrap ? "Disable Word Wrap" : "Enable Word Wrap")
    }

    private var collapseToggle: some View {
        HStack(spacing: 0) {
            Button {
                state.collapseAll()
            } label: {
                Image(systemName: "rectangle.compress.vertical")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.scaled(22), height: UIMetrics.controlSmall)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Collapse All Files")

            Button {
                state.expandAll()
            } label: {
                Image(systemName: "rectangle.expand.vertical")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.scaled(22), height: UIMetrics.controlSmall)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("Expand All Files")
        }
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
    }

    private var modeToggle: some View {
        HStack(spacing: 0) {
            modeButton(.split, symbol: "rectangle.split.2x1", tooltip: "Side by side")
            modeButton(.unified, symbol: "rectangle", tooltip: "Inline")
        }
        .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
        .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
    }

    private func modeButton(_ mode: VCSTabState.ViewMode, symbol: String, tooltip: String) -> some View {
        let selected = state.mode == mode
        return Button {
            state.mode = mode
        } label: {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(selected ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.scaled(22), height: UIMetrics.controlSmall)
                .background(selected ? MuxyTheme.bg : Color.clear)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(tooltip)
    }
}

private struct DiffCardList: View {
    @Bindable var state: DiffViewerTabState
    let sections: [DiffEditorFileSection]
    @State private var offsets: [String: CGFloat] = [:]

    private var cardMetrics: DiffCardMetrics {
        DiffCardMetrics(fontSize: state.fontSize, lineHeightMultiplier: EditorSettings.shared.lineHeightMultiplier)
    }

    private var cardSpacing: CGFloat {
        UIMetrics.spacing8
    }

    private func bottomScrollSpace(viewportHeight: CGFloat) -> CGFloat {
        max(0, viewportHeight - activeProbeY)
    }

    private var activeProbeY: CGFloat {
        UIMetrics.scaled(48)
    }

    var body: some View {
        GeometryReader { geometry in
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(spacing: cardSpacing) {
                        ForEach(sections, id: \.cacheKey) { section in
                            DiffFileCard(
                                state: state,
                                section: section,
                                viewportHeight: geometry.size.height,
                                metrics: cardMetrics
                            )
                            .id(section.cacheKey)
                            .background(sectionOffsetReader(section.cacheKey))
                        }
                        Color.clear
                            .frame(height: bottomScrollSpace(viewportHeight: geometry.size.height))
                    }
                    .padding(UIMetrics.spacing5)
                }
                .coordinateSpace(name: "diff-card-scroll")
                .onChange(of: state.scrollRequestVersion) { _, _ in
                    guard let cacheKey = state.selectedCacheKey else { return }
                    proxy.scrollTo(cacheKey, anchor: .top)
                }
                .onPreferenceChange(DiffCardOffsetPreferenceKey.self) { newOffsets in
                    offsets = newOffsets
                    let active = activeCacheKey(for: newOffsets)
                    state.activateFromDiffScroll(cacheKey: active)
                }
            }
        }
    }

    private func activeCacheKey(for offsets: [String: CGFloat]) -> String? {
        let probeY = activeProbeY
        return sections.first { section in
            guard let offset = offsets[section.cacheKey] else { return false }
            let extra = DiffCommentLayoutCache.layout(for: section, state: state).totalExtraHeight
            return offset <= probeY && offset + cardMetrics.cardHeight(for: section, extraCommentHeight: extra) >= probeY
        }?.cacheKey
    }

    private func sectionOffsetReader(_ cacheKey: String) -> some View {
        GeometryReader { proxy in
            Color.clear.preference(
                key: DiffCardOffsetPreferenceKey.self,
                value: [cacheKey: proxy.frame(in: .named("diff-card-scroll")).minY]
            )
        }
    }
}

private struct DiffFileCard: View {
    @Bindable var state: DiffViewerTabState
    let section: DiffEditorFileSection
    let viewportHeight: CGFloat
    let metrics: DiffCardMetrics

    private var isActive: Bool {
        state.activeCacheKey == section.cacheKey
    }

    private var layout: DiffCommentLayout {
        DiffCommentLayoutCache.layout(for: section, state: state)
    }

    private var cardHeight: CGFloat {
        metrics.cardHeight(for: section, extraCommentHeight: layout.totalExtraHeight)
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            if !section.isCollapsed {
                Rectangle().fill(MuxyTheme.border).frame(height: metrics.borderHeight)
                content
            }
        }
        .frame(height: cardHeight, alignment: .top)
        .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .stroke(isActive ? MuxyTheme.accent.opacity(0.45) : MuxyTheme.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    @ViewBuilder
    private var content: some View {
        if section.isLoading, section.rows.isEmpty {
            loadingBody
        } else if let errorMessage = section.errorMessage, section.rows.isEmpty {
            messageBody(errorMessage)
        } else if section.rows.isEmpty {
            emptyBody
        } else {
            columns
        }
    }

    @ViewBuilder
    private var columns: some View {
        switch state.mode {
        case .unified:
            DiffColumn(state: state, section: section, metrics: metrics, kind: .unified, layout: layout)
        case .split:
            HStack(spacing: 0) {
                DiffColumn(state: state, section: section, metrics: metrics, kind: .splitOld, layout: layout)
                Rectangle().fill(MuxyTheme.border).frame(width: 1)
                DiffColumn(state: state, section: section, metrics: metrics, kind: .splitNew, layout: layout)
            }
        }
    }

    private var loadingBody: some View {
        HStack(spacing: UIMetrics.spacing3) {
            ProgressView()
                .controlSize(.small)
            Text("Loading diff")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyBody: some View {
        VStack(spacing: UIMetrics.spacing4) {
            Text("Diff did not load")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)

            Button("Load diff") {
                state.loadDiff(filePath: section.filePath, isStaged: section.isStaged)
            }
            .buttonStyle(.plain)
            .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
            .foregroundStyle(MuxyTheme.accent)
            .padding(.horizontal, UIMetrics.spacing4)
            .frame(height: UIMetrics.controlSmall)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func messageBody(_ message: String) -> some View {
        Text(message)
            .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
            .foregroundStyle(MuxyTheme.diffRemoveFg)
            .lineLimit(3)
            .multilineTextAlignment(.center)
            .padding(UIMetrics.spacing5)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Button {
                state.toggleCollapsed(filePath: section.filePath, isStaged: section.isStaged)
            } label: {
                Image(systemName: section.isCollapsed ? "chevron.right" : "chevron.down")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(width: UIMetrics.iconSM, height: UIMetrics.iconSM)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(section.isCollapsed ? "Expand File" : "Collapse File")

            FileDiffIcon()
                .stroke(MuxyTheme.accent, style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round))
                .frame(width: UIMetrics.scaled(10), height: UIMetrics.scaled(10))

            Text(section.filePath)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
                .lineLimit(1)
                .truncationMode(.middle)

            if section.isStaged {
                Text("Staged")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .padding(.horizontal, UIMetrics.scaled(6))
                    .padding(.vertical, UIMetrics.scaled(1))
                    .background(MuxyTheme.surface, in: Capsule())
            }

            if section.additions > 0 {
                Text("+\(section.additions)")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                    .foregroundStyle(MuxyTheme.diffAddFg)
            }

            if section.deletions > 0 {
                Text("-\(section.deletions)")
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            }

            if section.isLargeUnloaded {
                Button("Load diff") {
                    state.loadFullDiff(filePath: section.filePath, isStaged: section.isStaged)
                }
                .buttonStyle(.plain)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .frame(height: metrics.headerHeight)
        .background(
            UnevenRoundedRectangle(
                topLeadingRadius: UIMetrics.radiusMD,
                bottomLeadingRadius: 0,
                bottomTrailingRadius: 0,
                topTrailingRadius: UIMetrics.radiusMD
            )
            .fill(MuxyTheme.surface)
        )
        .contentShape(Rectangle())
        .onTapGesture {
            state.toggleCollapsed(filePath: section.filePath, isStaged: section.isStaged)
        }
    }
}

private struct DiffCardSegmentRange: Equatable {
    let start: Int
    let end: Int
}

private enum DiffColumnKind {
    case unified
    case splitOld
    case splitNew

    var side: DiffCommentSide {
        self == .splitOld ? .old : .new
    }

    var rendersBothSides: Bool { self == .unified }
}

private struct DiffColumn: View {
    @Bindable var state: DiffViewerTabState
    let section: DiffEditorFileSection
    let metrics: DiffCardMetrics
    let kind: DiffColumnKind
    let layout: DiffCommentLayout

    @State private var hoveredRow: Int?
    @State private var dragSelection: ClosedRange<Int>?

    private var lineHeight: CGFloat {
        metrics.lineHeight()
    }

    private var topInset: CGFloat {
        DiffEditorLineMetrics.textContainerInset
    }

    private var renderedRows: [DiffRenderedRow] {
        DiffRenderedRowMapper.renderedRows(for: section.rows, mode: state.mode)
    }

    private var pairedRows: [SplitDiffPairedRow] {
        SplitDiffPairedRow.pair(section.rows)
    }

    private var gutterWidth: CGFloat {
        switch kind {
        case .unified:
            DiffGutterMetrics.width(rows: section.rows, fontSize: state.fontSize)
        case .splitOld:
            DiffGutterMetrics.width(pairedRows: pairedRows, side: .old, fontSize: state.fontSize)
        case .splitNew:
            DiffGutterMetrics.width(pairedRows: pairedRows, side: .new, fontSize: state.fontSize)
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            ForEach(segments, id: \.start) { segment in
                segmentEditor(range: segment.start ..< segment.end)
                if let gapHeight = layout.gaps[segment.end - 1] {
                    gapRegion(row: segment.end - 1, height: gapHeight)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .top)
    }

    private var segments: [DiffCardSegmentRange] {
        let rowCount = renderedRows.count
        guard rowCount > 0 else { return [] }
        let boundaries = layout.gaps.keys.sorted()
        var result: [DiffCardSegmentRange] = []
        var start = 0
        for boundary in boundaries {
            let end = min(rowCount, boundary + 1)
            guard end > start else { continue }
            result.append(DiffCardSegmentRange(start: start, end: end))
            start = end
        }
        if start < rowCount {
            result.append(DiffCardSegmentRange(start: start, end: rowCount))
        }
        return result
    }

    private func segmentEditor(range: Range<Int>) -> some View {
        let height = CGFloat(range.count) * lineHeight + topInset * 2
        return ZStack(alignment: .topLeading) {
            DiffColumnEditor(
                state: state,
                section: section,
                kind: kind,
                pairedRows: Array(pairedRows[safe: range]),
                rows: Array(section.rows[safe: rawRange(for: range)])
            )
            .frame(height: height)
            .clipped()

            overlay(range: range)
        }
        .frame(height: height)
    }

    private func rawRange(for renderedRange: Range<Int>) -> Range<Int> {
        guard kind.rendersBothSides else { return renderedRange }
        return renderedRange
    }

    private func overlay(range: Range<Int>) -> some View {
        ZStack(alignment: .topLeading) {
            if let selection = highlightRange, selection.overlaps(range) {
                let visibleLower = max(selection.lowerBound, range.lowerBound)
                let visibleUpper = min(selection.upperBound, range.upperBound - 1)
                Rectangle()
                    .fill(MuxyTheme.accent.opacity(0.14))
                    .frame(height: CGFloat(visibleUpper - visibleLower + 1) * lineHeight)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .offset(y: topInset + CGFloat(visibleLower - range.lowerBound) * lineHeight)
                    .allowsHitTesting(false)
            }
            if dragSelection == nil, activeComposerRange == nil, let row = hoveredRow, range.contains(row) {
                addButton(row: row - range.lowerBound)
            }
            hoverStrip(range: range)
        }
        .clipped()
    }

    private var activeComposerRange: ClosedRange<Int>? {
        guard let composer = state.activeComposer,
              composer.cacheKey == section.cacheKey,
              composer.side == kind.side || kind.rendersBothSides
        else { return nil }
        let lower = renderedRowIndex(side: composer.side, line: composer.startLine)
        let upper = renderedRowIndex(side: composer.side, line: composer.endLine)
        guard let lower, let upper else { return nil }
        return min(lower, upper) ... max(lower, upper)
    }

    private var highlightRange: ClosedRange<Int>? {
        dragSelection ?? activeComposerRange
    }

    private func renderedRowIndex(side: DiffCommentSide, line: Int) -> Int? {
        renderedRows.firstIndex { row in
            switch side {
            case .old: row.oldLineNumber == line
            case .new: row.newLineNumber == line
            }
        }
    }

    private func hoverStrip(range: Range<Int>) -> some View {
        Rectangle()
            .fill(Color.white.opacity(0.001))
            .frame(width: gutterWidth)
            .frame(maxHeight: .infinity, alignment: .topLeading)
            .onHover { inside in
                if inside { NSCursor.pointingHand.push() } else { NSCursor.pop() }
            }
            .onContinuousHover { phase in
                switch phase {
                case let .active(point):
                    if let row = commentableRow(localY: point.y, range: range) {
                        hoveredRow = row
                    }
                case .ended:
                    hoveredRow = nil
                }
            }
            .gesture(dragGesture(range: range))
    }

    private func addButton(row localRow: Int) -> some View {
        Image(systemName: "plus")
            .font(.system(size: UIMetrics.fontCaption, weight: .bold))
            .foregroundStyle(Color.white)
            .frame(width: UIMetrics.scaled(18), height: UIMetrics.scaled(18))
            .background(MuxyTheme.accent, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .offset(x: UIMetrics.scaled(4), y: topInset + CGFloat(localRow) * lineHeight + (lineHeight - UIMetrics.scaled(18)) / 2)
            .allowsHitTesting(false)
    }

    private func dragGesture(range: Range<Int>) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { value in
                guard let start = nearestCommentableRow(localY: value.startLocation.y, range: range),
                      let current = nearestCommentableRow(localY: value.location.y, range: range)
                else { return }
                dragSelection = min(start, current) ... max(start, current)
            }
            .onEnded { value in
                defer { dragSelection = nil }
                guard let start = nearestCommentableRow(localY: value.startLocation.y, range: range),
                      let current = nearestCommentableRow(localY: value.location.y, range: range)
                else { return }
                openComposer(range: min(start, current) ... max(start, current))
            }
    }

    private func commentableRow(localY: CGFloat, range: Range<Int>) -> Int? {
        guard lineHeight > 0 else { return nil }
        let row = range.lowerBound + Int((localY - topInset) / lineHeight)
        guard range.contains(row), lineNumber(at: row) != nil else { return nil }
        return row
    }

    private func nearestCommentableRow(localY: CGFloat, range: Range<Int>) -> Int? {
        guard lineHeight > 0, !range.isEmpty else { return nil }
        let raw = min(range.upperBound - 1, max(range.lowerBound, range.lowerBound + Int((localY - topInset) / lineHeight)))
        if lineNumber(at: raw) != nil { return raw }
        for delta in 1 ..< max(2, range.count) {
            if raw - delta >= range.lowerBound, lineNumber(at: raw - delta) != nil { return raw - delta }
            if raw + delta < range.upperBound, lineNumber(at: raw + delta) != nil { return raw + delta }
        }
        return nil
    }

    private func lineNumber(at renderedRow: Int) -> Int? {
        guard renderedRow >= 0, renderedRow < renderedRows.count else { return nil }
        let row = renderedRows[renderedRow]
        if kind.rendersBothSides {
            return row.newLineNumber ?? row.oldLineNumber
        }
        return kind.side == .new ? row.newLineNumber : row.oldLineNumber
    }

    private func sideForComposer(at renderedRow: Int) -> DiffCommentSide {
        guard kind.rendersBothSides else { return kind.side }
        guard renderedRow >= 0, renderedRow < renderedRows.count else { return .new }
        return renderedRows[renderedRow].newLineNumber != nil ? .new : .old
    }

    private func openComposer(range: ClosedRange<Int>) {
        let side = sideForComposer(at: range.lowerBound)
        let lines = range.compactMap { renderedRow -> Int? in
            guard renderedRow >= 0, renderedRow < renderedRows.count else { return nil }
            let row = renderedRows[renderedRow]
            return side == .new ? row.newLineNumber : row.oldLineNumber
        }
        guard let startLine = lines.first, let endLine = lines.last else { return }
        state.beginComposer(
            cacheKey: section.cacheKey,
            filePath: section.filePath,
            side: side,
            startLine: startLine,
            endLine: endLine
        )
    }

    @ViewBuilder
    private func gapRegion(row: Int, height: CGFloat) -> some View {
        let blocks = layout.blocks(side: kind.side, atRow: row)
            + (kind.rendersBothSides ? layout.blocks(side: .old, atRow: row) : [])
        VStack(spacing: 0) {
            ForEach(blocks) { block in
                commentBox(block)
            }
            Spacer(minLength: 0)
        }
        .frame(height: height, alignment: .top)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(gutterContinuation)
    }

    private var gutterContinuation: some View {
        Color(EditorThemePalette.active.background)
            .frame(width: gutterWidth)
            .overlay(alignment: .trailing) {
                Color(EditorThemePalette.active.foreground.withAlphaComponent(0.08))
                    .frame(width: 1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func commentBox(_ block: DiffCommentBlock) -> some View {
        Group {
            switch block.content {
            case let .composer(target):
                DiffCommentComposer(state: state, target: target)
            case let .comment(comment):
                DiffCommentThread(state: state, comment: comment)
            }
        }
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .frame(height: block.height, alignment: .top)
        .padding(.leading, gutterWidth + DiffCommentMetrics.blockHorizontalInset)
        .padding(.trailing, DiffCommentMetrics.blockHorizontalInset)
        .padding(.vertical, DiffCommentLayout.verticalInset)
    }
}

private struct DiffColumnEditor: View {
    @Bindable var state: DiffViewerTabState
    let section: DiffEditorFileSection
    let kind: DiffColumnKind
    let pairedRows: [SplitDiffPairedRow]
    let rows: [DiffDisplayRow]

    @State private var editorSettings = EditorSettings.shared
    @State private var themeRevision = 0
    @State private var documentRevision = 0
    @State private var editorState: EditorTabState
    @State private var appliedSignature = ""

    init(
        state: DiffViewerTabState,
        section: DiffEditorFileSection,
        kind: DiffColumnKind,
        pairedRows: [SplitDiffPairedRow],
        rows: [DiffDisplayRow]
    ) {
        self.state = state
        self.section = section
        self.kind = kind
        self.pairedRows = pairedRows
        self.rows = rows
        _editorState = State(initialValue: EditorTabState(
            projectPath: state.projectPath,
            filePath: section.filePath,
            readOnlyText: "",
            diffLineKinds: []
        ))
    }

    var body: some View {
        CodeEditorView(
            state: editorState,
            editorSettings: editorSettings,
            fontFamilyOverride: "SF Mono",
            fontSizeOverride: state.fontSize,
            showLineNumbers: false,
            lineWrapping: state.wordWrap,
            themeVersion: GhosttyService.shared.configVersion + themeRevision + documentRevision,
            showsVerticalScroller: false,
            focused: false,
            searchNeedle: "",
            searchNavigationVersion: 0,
            searchNavigationDirection: .next,
            searchCaseSensitive: false,
            searchUseRegex: false,
            replaceText: "",
            replaceVersion: 0,
            replaceAllVersion: 0,
            editorFocusVersion: 0,
            synchronizedScrollY: nil,
            passesScrollWheelToParent: true,
            onFocus: {}
        )
        .background(MuxyTheme.bg)
        .onAppear(perform: sync)
        .onChange(of: signature) { _, _ in sync() }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in themeRevision &+= 1 }
    }

    private var maxLineCharacters: Int {
        state.wordWrap ? 1024 : 2048
    }

    private var signature: String {
        let count = kind.rendersBothSides ? rows.count : pairedRows.count
        let firstID = kind.rendersBothSides ? rows.first?.id.uuidString : pairedRows.first?.id.uuidString
        let lastID = kind.rendersBothSides ? rows.last?.id.uuidString : pairedRows.last?.id.uuidString
        return "\(kind):\(count):\(firstID ?? ""):\(lastID ?? ""):\(maxLineCharacters)"
    }

    private func sync() {
        guard appliedSignature != signature else { return }
        let options = DiffEditorDocument.RenderOptions(maxLineCharacters: maxLineCharacters)
        let document: DiffEditorDocument = switch kind {
        case .unified: .unified(rows: rows, options: options)
        case .splitOld: .splitLeft(paired: pairedRows, options: options)
        case .splitNew: .splitRight(paired: pairedRows, options: options)
        }
        editorState.replaceReadOnlyText(
            document.text,
            filePath: section.filePath,
            diffLineKinds: document.lineKinds,
            diffGutterLines: document.gutterLines
        )
        documentRevision &+= 1
        appliedSignature = signature
    }
}

private extension Array {
    subscript(safe range: Range<Int>) -> ArraySlice<Element> {
        let lower = Swift.max(0, range.lowerBound)
        let upper = Swift.min(count, range.upperBound)
        guard lower < upper else { return self[0 ..< 0] }
        return self[lower ..< upper]
    }
}

enum DiffCardLineCount {
    static func value(for section: DiffEditorFileSection) -> Int {
        guard section.rows.isEmpty else { return section.rows.count }
        return max(1, section.additions + section.deletions)
    }
}

enum DiffEditorLineMetrics {
    static let textContainerInset: CGFloat = 4
    static let fontFamily = "SF Mono"

    static func lineHeight(fontSize: CGFloat, lineHeightMultiplier: CGFloat) -> CGFloat {
        let font = NSFont(name: fontFamily, size: fontSize)
            ?? NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        let typographicHeight = font.ascender - font.descender
        let scaled = ceil(typographicHeight * lineHeightMultiplier)
        return scaled > 0 ? scaled : 16
    }

    static func editorHeight(lineCount: Int, fontSize: CGFloat, lineHeightMultiplier: CGFloat) -> CGFloat {
        let height = CGFloat(lineCount) * lineHeight(fontSize: fontSize, lineHeightMultiplier: lineHeightMultiplier)
        return height + textContainerInset * 2
    }
}

@MainActor
private struct DiffCardMetrics {
    let fontSize: CGFloat
    let lineHeightMultiplier: CGFloat

    var headerHeight: CGFloat {
        UIMetrics.scaled(36)
    }

    var borderHeight: CGFloat {
        UIMetrics.scaled(1)
    }

    func lineHeight() -> CGFloat {
        DiffEditorLineMetrics.lineHeight(fontSize: fontSize, lineHeightMultiplier: lineHeightMultiplier)
    }

    func editorHeight(for section: DiffEditorFileSection, extraCommentHeight: CGFloat = 0) -> CGFloat {
        let lineCount = DiffCardLineCount.value(for: section)
        let height = DiffEditorLineMetrics.editorHeight(
            lineCount: lineCount,
            fontSize: fontSize,
            lineHeightMultiplier: lineHeightMultiplier
        )
        return max(UIMetrics.scaled(80), height) + extraCommentHeight
    }

    func cardHeight(for section: DiffEditorFileSection, extraCommentHeight: CGFloat = 0) -> CGFloat {
        headerHeight + (section.isCollapsed ? 0 : editorHeight(for: section, extraCommentHeight: extraCommentHeight) + borderHeight)
    }
}

@MainActor
private enum DiffCommentLayoutCache {
    static func layout(
        for section: DiffEditorFileSection,
        state: DiffViewerTabState
    ) -> DiffCommentLayout {
        let comments = state.comments(forCacheKey: section.cacheKey)
        let composer = state.activeComposer?.cacheKey == section.cacheKey ? state.activeComposer : nil
        guard !comments.isEmpty || composer != nil else { return .empty }
        let renderedRows = DiffRenderedRowMapper.renderedRows(for: section.rows, mode: state.mode)
        return DiffCommentLayout.make(
            renderedRows: renderedRows,
            comments: comments,
            composer: composer,
            composerLineCount: state.composerLineCount
        )
    }
}

private struct DiffCardOffsetPreferenceKey: PreferenceKey {
    static let defaultValue: [String: CGFloat] = [:]

    static func reduce(value: inout [String: CGFloat], nextValue: () -> [String: CGFloat]) {
        value.merge(nextValue(), uniquingKeysWith: { _, new in new })
    }
}

private struct DiffViewerSidebar: View {
    @Bindable var state: DiffViewerTabState

    var body: some View {
        VStack(spacing: 0) {
            header
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(spacing: 0) {
                        if !state.stagedFiles.isEmpty {
                            DiffViewerSidebarSection(state: state, title: "Staged", files: state.stagedFiles, isStaged: true)
                        }
                        DiffViewerSidebarSection(state: state, title: "Changes", files: state.unstagedFiles, isStaged: false)
                    }
                }
                .onChange(of: state.sidebarScrollRequestVersion) { _, _ in
                    guard let cacheKey = state.activeCacheKey else { return }
                    proxy.scrollTo(cacheKey, anchor: .center)
                }
            }
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            DiffViewerStats(stagedFiles: state.stagedFiles, unstagedFiles: state.unstagedFiles)
        }
        .background(MuxyTheme.bg)
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: "list.bullet.rectangle")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)

            Text("Diff Files")
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)

            Text("\(state.stagedFiles.count + state.unstagedFiles.count)")
                .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                .foregroundStyle(MuxyTheme.bg)
                .padding(.horizontal, UIMetrics.spacing3)
                .padding(.vertical, UIMetrics.scaled(1))
                .background(MuxyTheme.fgMuted, in: Capsule())

            Spacer(minLength: 0)

            if state.source == .workingTree {
                Button {
                    state.vcs.fileListMode = state.vcs.fileListMode == .flat ? .folders : .flat
                } label: {
                    Image(systemName: state.vcs.fileListMode == .flat ? "folder" : "list.bullet")
                        .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgMuted)
                        .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(state.vcs.fileListMode == .flat ? "Switch to Folder View" : "Switch to Flat View")
            }
        }
        .padding(.horizontal, UIMetrics.spacing4)
        .frame(height: UIMetrics.scaled(32))
    }
}

private struct DiffViewerSidebarSection: View {
    @Bindable var state: DiffViewerTabState
    let title: String
    let files: [GitStatusFile]
    let isStaged: Bool

    var body: some View {
        if !files.isEmpty {
            VStack(spacing: 0) {
                HStack(spacing: UIMetrics.spacing3) {
                    Text(title)
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                        .foregroundStyle(MuxyTheme.fgDim)
                    Spacer(minLength: 0)
                    Text("\(files.count)")
                        .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                .padding(.horizontal, UIMetrics.spacing4)
                .frame(height: UIMetrics.scaled(26))

                if state.source != .workingTree || state.vcs.fileListMode == .flat {
                    ForEach(files, id: \.path) { file in
                        DiffViewerSidebarFileRow(state: state, file: file, isStaged: isStaged, displayPath: file.path, depth: 0)
                    }
                } else {
                    ForEach(rows) { row in
                        switch row {
                        case let .folder(folder):
                            DiffViewerSidebarFolderRow(state: state, folder: folder, isStaged: isStaged)
                        case let .file(file, depth):
                            DiffViewerSidebarFileRow(
                                state: state,
                                file: file,
                                isStaged: isStaged,
                                displayPath: (file.path as NSString).lastPathComponent,
                                depth: depth
                            )
                        }
                    }
                }
            }
        }
    }

    private var rows: [VCSFileTree.Row] {
        isStaged ? state.vcs.stagedTreeRows : state.vcs.unstagedTreeRows
    }
}

private struct DiffViewerSidebarFolderRow: View {
    @Bindable var state: DiffViewerTabState
    let folder: VCSFileTree.Folder
    let isStaged: Bool

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: state.vcs.isFolderExpanded(folder.path, isStaged: isStaged) ? "chevron.down" : "chevron.right")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(width: UIMetrics.iconSM)

            Image(systemName: "folder")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)

            Text(folder.name)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)

            Spacer(minLength: 0)
        }
        .padding(.leading, UIMetrics.spacing4 + CGFloat(folder.depth) * UIMetrics.iconMD)
        .padding(.trailing, UIMetrics.spacing4)
        .frame(height: UIMetrics.scaled(28))
        .contentShape(Rectangle())
        .onTapGesture {
            state.vcs.toggleFolderExpanded(folder.path, isStaged: isStaged)
        }
    }
}

private struct DiffViewerSidebarFileRow: View {
    @Bindable var state: DiffViewerTabState
    let file: GitStatusFile
    let isStaged: Bool
    let displayPath: String
    let depth: Int

    private var selected: Bool {
        state.activeCacheKey == DiffViewerTabState.cacheKey(filePath: file.path, isStaged: isStaged)
    }

    private var statusText: String {
        file.displayStatusText(isStaged: isStaged)
    }

    private var statusColor: Color {
        switch statusText.first {
        case "A",
             "U": MuxyTheme.diffAddFg
        case "D": MuxyTheme.diffRemoveFg
        case "M",
             "R": MuxyTheme.accent
        default: MuxyTheme.fgMuted
        }
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Text(statusText)
                .font(.system(size: UIMetrics.fontCaption, weight: .bold, design: .monospaced))
                .foregroundStyle(statusColor)
                .frame(width: UIMetrics.iconSM)

            FileDiffIcon()
                .stroke(statusColor, style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round))
                .frame(width: UIMetrics.scaled(10), height: UIMetrics.scaled(10))

            Text(displayPath)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(selected ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)

            Spacer(minLength: 0)

            if let additions = file.additions(isStaged: isStaged), additions > 0 {
                Text("+\(additions)")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                    .foregroundStyle(MuxyTheme.diffAddFg)
            }
            if let deletions = file.deletions(isStaged: isStaged), deletions > 0 {
                Text("-\(deletions)")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
            }
        }
        .padding(.leading, UIMetrics.spacing3 + CGFloat(depth) * UIMetrics.iconMD)
        .padding(.trailing, UIMetrics.spacing4)
        .frame(height: UIMetrics.scaled(30))
        .background(selected ? MuxyTheme.surface : Color.clear)
        .contentShape(Rectangle())
        .id(DiffViewerTabState.cacheKey(filePath: file.path, isStaged: isStaged))
        .onTapGesture {
            state.select(filePath: file.path, isStaged: isStaged)
        }
    }
}

private struct DiffViewerStats: View {
    let stagedFiles: [GitStatusFile]
    let unstagedFiles: [GitStatusFile]

    private var additions: Int {
        stagedFiles.compactMap { $0.additions(isStaged: true) }.reduce(0, +)
            + unstagedFiles.compactMap { $0.additions(isStaged: false) }.reduce(0, +)
    }

    private var deletions: Int {
        stagedFiles.compactMap { $0.deletions(isStaged: true) }.reduce(0, +)
            + unstagedFiles.compactMap { $0.deletions(isStaged: false) }.reduce(0, +)
    }

    private var fileCount: Int {
        stagedFiles.count + unstagedFiles.count
    }

    var body: some View {
        VStack(spacing: UIMetrics.spacing3) {
            statRow("Files", value: "\(fileCount)", color: MuxyTheme.fg)
            statRow("Additions", value: "+\(additions)", color: MuxyTheme.diffAddFg)
            statRow("Deletions", value: "-\(deletions)", color: MuxyTheme.diffRemoveFg)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }

    private func statRow(_ label: String, value: String, color: Color) -> some View {
        HStack {
            Text(label)
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(MuxyTheme.fgDim)
            Spacer(minLength: 0)
            Text(value)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                .foregroundStyle(color)
        }
    }
}
