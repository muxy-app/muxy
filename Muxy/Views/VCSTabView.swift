import AppKit
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

private struct UnifiedDiffView: View {
    let rows: [DiffDisplayRow]
    let filePath: String

    var body: some View {
        LazyVStack(spacing: 0) {
            ForEach(rows) { row in
                if row.kind == .hunk || row.kind == .collapsed {
                    DiffSectionDivider(text: row.kind == .hunk ? hunkLabel(row.text) : row.text)
                } else {
                    DiffLineRow(filePath: filePath, lineNumber: row.newLineNumber ?? row.oldLineNumber) {
                        HStack(spacing: 0) {
                            numberCell(row.oldLineNumber)
                            numberCell(row.newLineNumber)
                            lineContent(row)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, 8)
                        }
                        .frame(minHeight: 24)
                        .background(rowBackground(row.kind, side: .both))
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func numberCell(_ number: Int?) -> some View {
        Text(number.map(String.init) ?? "")
            .font(.system(size: 11, design: .monospaced))
            .foregroundStyle(MuxyTheme.fgDim)
            .frame(width: 42, alignment: .trailing)
            .padding(.trailing, 6)
            .background(.clear)
            .overlay(alignment: .trailing) {
                Rectangle().fill(MuxyTheme.border).frame(width: 1)
            }
    }

    @ViewBuilder
    private func lineContent(_ row: DiffDisplayRow) -> some View {
        switch row.kind {
        case .context:
            CodeHighlightedText(text: row.newText ?? "", kind: .context)
                .padding(.vertical, 2)
        case .addition:
            CodeHighlightedText(text: row.newText ?? "", kind: .addition)
                .padding(.vertical, 2)
        case .deletion:
            CodeHighlightedText(text: row.oldText ?? "", kind: .deletion)
                .padding(.vertical, 2)
        case .hunk,
             .collapsed:
            EmptyView()
        }
    }
}

private struct SplitDiffView: View {
    let rows: [DiffDisplayRow]
    let filePath: String
    let pairedRows: [SplitDiffPairedRow]

    init(rows: [DiffDisplayRow], filePath: String) {
        self.rows = rows
        self.filePath = filePath
        pairedRows = SplitDiffPairedRow.pair(rows)
    }

    var body: some View {
        LazyVStack(spacing: 0) {
            ForEach(pairedRows) { paired in
                switch paired.kind {
                case .hunk,
                     .collapsed:
                    hunkOrCollapsedRow(paired)
                case .content:
                    contentRow(paired)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func hunkOrCollapsedRow(_ paired: SplitDiffPairedRow) -> some View {
        let rawText = paired.left?.text ?? paired.right?.text ?? ""
        let label = paired.kind == .hunk ? hunkLabel(rawText) : rawText
        return DiffSectionDivider(text: label)
    }

    private func contentRow(_ paired: SplitDiffPairedRow) -> some View {
        let lineNumber = paired.right?.newLineNumber ?? paired.left?.oldLineNumber
        return DiffLineRow(filePath: filePath, lineNumber: lineNumber) {
            HStack(spacing: 0) {
                splitCell(
                    number: paired.left?.oldLineNumber,
                    text: paired.left?.oldText,
                    changeKind: paired.left?.kind ?? .context,
                    isLeft: true
                )
                Rectangle().fill(MuxyTheme.border).frame(width: 1)
                splitCell(
                    number: paired.right?.newLineNumber,
                    text: paired.right?.newText,
                    changeKind: paired.right?.kind ?? .context,
                    isLeft: false
                )
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func splitCell(
        number: Int?,
        text: String?,
        changeKind: DiffDisplayRow.Kind,
        isLeft: Bool
    ) -> some View {
        let highlightKind: CodeHighlightedText.ChangeKind = switch changeKind {
        case .deletion: .deletion
        case .addition: .addition
        default: .context
        }
        let bgKind: DiffDisplayRow.Kind = isLeft
            ? (changeKind == .deletion ? .deletion : .context)
            : (changeKind == .addition ? .addition : .context)

        return HStack(spacing: 0) {
            Text(number.map(String.init) ?? "")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(width: 42, alignment: .trailing)
                .padding(.trailing, 6)
                .background(.clear)
                .overlay(alignment: .trailing) {
                    Rectangle().fill(MuxyTheme.border).frame(width: 1)
                }

            CodeHighlightedText(text: text ?? "", kind: highlightKind)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 8)
                .padding(.vertical, 2)
        }
        .background(rowBackground(bgKind, side: isLeft ? .left : .right))
    }
}

private struct DiffLineRow<Content: View>: View {
    let filePath: String
    let lineNumber: Int?
    @ViewBuilder let content: Content
    @State private var hovered = false

    var body: some View {
        content
            .overlay(alignment: .leading) {
                if hovered, let lineNumber {
                    Menu {
                        Button("Copy Reference") {
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString("\(filePath):\(lineNumber)", forType: .string)
                        }
                    } label: {
                        Image(systemName: "ellipsis")
                            .font(.system(size: 9, weight: .bold))
                            .foregroundStyle(MuxyTheme.fgMuted)
                            .frame(width: 20, height: 20)
                            .background(
                                RoundedRectangle(cornerRadius: 4)
                                    .fill(MuxyTheme.surface)
                            )
                    }
                    .menuStyle(.borderlessButton)
                    .menuIndicator(.hidden)
                    .frame(width: 20)
                    .padding(.leading, 2)
                }
            }
            .onHover { hovered = $0 }
    }
}

private struct SplitDiffPairedRow: Identifiable {
    enum Kind {
        case content
        case hunk
        case collapsed
    }

    let id = UUID()
    let kind: Kind
    let left: DiffDisplayRow?
    let right: DiffDisplayRow?

    static func pair(_ rows: [DiffDisplayRow]) -> [SplitDiffPairedRow] {
        var result: [SplitDiffPairedRow] = []
        var index = 0

        while index < rows.count {
            let row = rows[index]

            switch row.kind {
            case .hunk:
                result.append(SplitDiffPairedRow(kind: .hunk, left: row, right: nil))
                index += 1

            case .collapsed:
                result.append(SplitDiffPairedRow(kind: .collapsed, left: row, right: nil))
                index += 1

            case .context:
                result.append(SplitDiffPairedRow(kind: .content, left: row, right: row))
                index += 1

            case .deletion:
                var deletions: [DiffDisplayRow] = []
                while index < rows.count, rows[index].kind == .deletion {
                    deletions.append(rows[index])
                    index += 1
                }
                var additions: [DiffDisplayRow] = []
                while index < rows.count, rows[index].kind == .addition {
                    additions.append(rows[index])
                    index += 1
                }
                let maxCount = max(deletions.count, additions.count)
                for i in 0 ..< maxCount {
                    result.append(SplitDiffPairedRow(
                        kind: .content,
                        left: i < deletions.count ? deletions[i] : nil,
                        right: i < additions.count ? additions[i] : nil
                    ))
                }

            case .addition:
                result.append(SplitDiffPairedRow(kind: .content, left: nil, right: row))
                index += 1
            }
        }

        return result
    }
}

private struct DiffSectionDivider: View {
    let text: String

    var body: some View {
        HStack(spacing: 0) {
            Text(text)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .lineLimit(1)
                .truncationMode(.tail)
                .padding(.leading, 10)
            Spacer(minLength: 8)
        }
        .frame(height: 28)
        .frame(maxWidth: .infinity)
        .background(MuxyTheme.bg)
        .overlay(alignment: .top) {
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
        .overlay(alignment: .bottom) {
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
        }
    }
}

private func hunkLabel(_ raw: String) -> String {
    guard raw.count > 2,
          let closingRange = raw.range(of: "@@", range: raw.index(raw.startIndex, offsetBy: 2) ..< raw.endIndex)
    else { return raw }
    let after = raw[closingRange.upperBound...].trimmingCharacters(in: .whitespaces)
    return after.isEmpty ? raw : after
}

private enum DiffBackgroundSide {
    case left
    case right
    case both
}

@MainActor
private func rowBackground(_ kind: DiffDisplayRow.Kind, side: DiffBackgroundSide) -> Color {
    switch kind {
    case .addition:
        switch side {
        case .left:
            .clear
        case .right,
             .both:
            MuxyTheme.diffAddBg
        }
    case .deletion:
        switch side {
        case .left,
             .both:
            MuxyTheme.diffRemoveBg
        case .right:
            .clear
        }
    case .hunk:
        MuxyTheme.diffHunkBg
    case .collapsed:
        MuxyTheme.bg
    case .context:
        .clear
    }
}

private struct CodeHighlightedText: View {
    enum ChangeKind {
        case context
        case addition
        case deletion
    }

    let text: String
    let kind: ChangeKind

    var body: some View {
        Text(DiffHighlightCache.shared.highlighted(text, kind: kind))
            .font(.system(size: 12, design: .monospaced))
            .lineLimit(1)
            .truncationMode(.tail)
            .textSelection(.enabled)
    }
}

@MainActor
private final class DiffHighlightCache {
    static let shared = DiffHighlightCache()

    private struct CacheKey: Hashable {
        let text: String
        let kind: CodeHighlightedText.ChangeKind
    }

    private var cache: [CacheKey: AttributedString] = [:]
    private var insertionOrder: [CacheKey] = []
    private let maxEntries = 2000

    struct Rule {
        let regex: NSRegularExpression
        let color: @MainActor () -> NSColor
    }

    let rules: [Rule]

    private init() {
        rules = Self.buildRules()
    }

    func highlighted(_ source: String, kind: CodeHighlightedText.ChangeKind) -> AttributedString {
        let key = CacheKey(text: source, kind: kind)
        if let cached = cache[key] {
            return cached
        }
        let result = computeHighlighted(source, kind: kind)
        if insertionOrder.count >= maxEntries {
            let evicted = insertionOrder.removeFirst()
            cache.removeValue(forKey: evicted)
        }
        cache[key] = result
        insertionOrder.append(key)
        return result
    }

    func invalidate() {
        cache.removeAll()
        insertionOrder.removeAll()
    }

    private func computeHighlighted(_ source: String, kind: CodeHighlightedText.ChangeKind) -> AttributedString {
        let fullRange = NSRange(location: 0, length: (source as NSString).length)

        let baseColor: NSColor = switch kind {
        case .addition: MuxyTheme.nsDiffAdd
        case .deletion: MuxyTheme.nsDiffRemove
        case .context: GhosttyService.shared.foregroundColor
        }

        let attributed = NSMutableAttributedString(
            string: source,
            attributes: [
                .foregroundColor: baseColor,
                .font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular),
            ]
        )

        for rule in rules {
            let matches = rule.regex.matches(in: source, range: fullRange)
            let color = rule.color()
            for match in matches {
                attributed.addAttribute(.foregroundColor, value: color, range: match.range)
            }
        }

        return AttributedString(attributed)
    }

    private struct RuleDefinition {
        let pattern: String
        let color: @MainActor () -> NSColor
        let options: NSRegularExpression.Options
    }

    private static func buildRules() -> [Rule] {
        var result: [Rule] = []

        let definitions: [RuleDefinition] = [
            RuleDefinition(pattern: #"'(?:\\.|[^'\\])*'"#, color: { MuxyTheme.nsDiffString }, options: []),
            RuleDefinition(pattern: #""(?:\\.|[^"\\])*""#, color: { MuxyTheme.nsDiffString }, options: []),
            RuleDefinition(pattern: #"`(?:\\.|[^`\\])*`"#, color: { MuxyTheme.nsDiffString }, options: []),
            RuleDefinition(pattern: #"\b\d+(?:\.\d+)?\b"#, color: { MuxyTheme.nsDiffNumber }, options: []),
            RuleDefinition(pattern: #"//.*$"#, color: { MuxyTheme.nsDiffComment }, options: [.anchorsMatchLines]),
        ]

        for definition in definitions {
            guard let regex = try? NSRegularExpression(pattern: definition.pattern, options: definition.options)
            else { continue }
            result.append(Rule(regex: regex, color: definition.color))
        }

        return result
    }
}

private struct PRBadge: View {
    let info: GitRepositoryService.PRInfo
    @State private var hovered = false

    var body: some View {
        Button {
            NSWorkspace.shared.open(URL(string: info.url)!)
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "arrow.triangle.pull")
                    .font(.system(size: 9, weight: .bold))
                Text("#\(info.number)")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
            }
            .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(MuxyTheme.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 5)
                    .stroke(MuxyTheme.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}
