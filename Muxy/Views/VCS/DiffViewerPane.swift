import SwiftUI

struct DiffViewerPane: View {
    @Bindable var state: DiffViewerTabState
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
            if state.source == .workingTree, !state.vcs.hasCompletedInitialLoad, !state.vcs.isLoadingFiles {
                state.vcs.refresh()
            }
            state.reconcileSelection()
            state.loadAllDiffs()
        }
        .onChange(of: state.vcs.files) { _, _ in
            guard state.source == .workingTree else { return }
            state.reconcileSelection()
            state.loadAllDiffs()
        }
        .onChange(of: state.vcs.diffCache.revision) { _, _ in
            guard state.source == .workingTree else { return }
            state.reconcileLargeDiffCollapse()
        }
        .onChange(of: state.diffCache.revision) { _, _ in
            guard state.source != .workingTree else { return }
            state.reconcileLargeDiffCollapse()
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
        } else if isLoadingAnyDiff {
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
                additions: diff?.additions ?? file.additions ?? 0,
                deletions: diff?.deletions ?? file.deletions ?? 0,
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
        state.files.compactMap(\.additions).reduce(0, +)
    }

    private var deletions: Int {
        state.files.compactMap(\.deletions).reduce(0, +)
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

            Text("\(state.files.count) files")
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
        DiffCardMetrics(fontSize: state.fontSize)
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
                                cardOffsetY: offsets[section.cacheKey] ?? 0,
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
            return offset <= probeY && offset + cardMetrics.cardHeight(for: section) >= probeY
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
    let cardOffsetY: CGFloat
    let viewportHeight: CGFloat
    let metrics: DiffCardMetrics

    private var isActive: Bool {
        state.activeCacheKey == section.cacheKey
    }

    private var editorHeight: CGFloat {
        metrics.editorHeight(for: section)
    }

    private var cardHeight: CGFloat {
        metrics.cardHeight(for: section)
    }

    private var usesVirtualBody: Bool {
        editorHeight > max(viewportHeight * 1.5, UIMetrics.scaled(900))
    }

    private var editorViewportHeight: CGFloat {
        min(editorHeight, max(UIMetrics.scaled(160), viewportHeight + UIMetrics.scaled(80)))
    }

    private var bodyScrollY: CGFloat {
        guard usesVirtualBody else { return 0 }
        let headerHeight = UIMetrics.scaled(37)
        let visibleBodyY = max(0, -cardOffsetY - headerHeight)
        return min(max(0, editorHeight - editorViewportHeight), visibleBodyY)
    }

    var body: some View {
        ZStack(alignment: .top) {
            VStack(spacing: 0) {
                header
                if !section.isCollapsed {
                    Rectangle().fill(MuxyTheme.border).frame(height: metrics.borderHeight)
                    editorBody
                }
            }
            Color.clear.frame(height: cardHeight)
        }
        .frame(height: cardHeight)
        .background(MuxyTheme.bg, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        .overlay(
            RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                .stroke(isActive ? MuxyTheme.accent.opacity(0.45) : MuxyTheme.border, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
    }

    @ViewBuilder
    private var editorBody: some View {
        if usesVirtualBody {
            ZStack(alignment: .top) {
                Color.clear.frame(height: editorHeight)
                editor(externalScrollY: bodyScrollY)
                    .frame(height: editorViewportHeight)
                    .offset(y: bodyScrollY)
            }
            .frame(height: editorHeight)
            .clipped()
        } else {
            editor(externalScrollY: nil)
                .frame(height: editorHeight)
                .clipped()
        }
    }

    private func editor(externalScrollY: CGFloat?) -> some View {
        SingleDiffEditorView(
            rows: section.rows,
            projectPath: state.projectPath,
            filePath: section.filePath,
            cacheKey: section.cacheKey,
            mode: state.mode,
            wordWrap: state.wordWrap,
            fontSize: state.fontSize,
            maxLineCharacters: maxRenderedCharacters,
            externalScrollY: externalScrollY,
            passesScrollWheelToParent: true
        )
        .id("\(section.cacheKey):\(state.mode.rawValue):\(state.wordWrap):\(state.fontSize):\(usesVirtualBody):\(maxRenderedCharacters)")
        .frame(maxWidth: .infinity)
    }

    private var maxRenderedCharacters: Int {
        state.wordWrap ? 1024 : 2048
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
            state.select(filePath: section.filePath, isStaged: section.isStaged)
        }
    }
}

@MainActor
private struct DiffCardMetrics {
    let fontSize: CGFloat

    var headerHeight: CGFloat {
        UIMetrics.scaled(36)
    }

    var borderHeight: CGFloat {
        UIMetrics.scaled(1)
    }

    func editorHeight(for section: DiffEditorFileSection) -> CGFloat {
        let lineHeight = max(18, fontSize * 1.45)
        let rowCount = max(1, section.rows.count)
        return max(UIMetrics.scaled(80), CGFloat(rowCount) * lineHeight + UIMetrics.scaled(18))
    }

    func cardHeight(for section: DiffEditorFileSection) -> CGFloat {
        headerHeight + (section.isCollapsed ? 0 : editorHeight(for: section) + borderHeight)
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
            DiffViewerStats(files: state.files)
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

            Text("\(state.files.count)")
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
                    ForEach(files) { file in
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
        isStaged ? file.stagedStatusText : file.unstagedStatusText
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

            if let additions = file.additions, additions > 0 {
                Text("+\(additions)")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                    .foregroundStyle(MuxyTheme.diffAddFg)
            }
            if let deletions = file.deletions, deletions > 0 {
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
    let files: [GitStatusFile]

    private var additions: Int {
        files.compactMap(\.additions).reduce(0, +)
    }

    private var deletions: Int {
        files.compactMap(\.deletions).reduce(0, +)
    }

    var body: some View {
        VStack(spacing: UIMetrics.spacing3) {
            statRow("Files", value: "\(files.count)", color: MuxyTheme.fg)
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
