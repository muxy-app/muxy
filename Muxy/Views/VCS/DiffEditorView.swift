import SwiftUI

struct DiffEditorView: View {
    let sections: [DiffEditorFileSection]
    let projectPath: String
    let cacheKey: String
    let mode: VCSTabState.ViewMode
    let wordWrap: Bool
    let fontSize: CGFloat
    let scrollTargetCacheKey: String?
    let scrollRequestVersion: Int

    @State private var editorSettings = EditorSettings.shared
    @State private var themeRevision = 0
    @State private var splitScrollY: CGFloat = 0
    @State private var unifiedState: EditorTabState
    @State private var leftState: EditorTabState
    @State private var rightState: EditorTabState

    init(
        rows: [DiffDisplayRow],
        projectPath: String,
        filePath: String,
        cacheKey: String,
        mode: VCSTabState.ViewMode,
        wordWrap: Bool,
        fontSize: CGFloat
    ) {
        let section = DiffEditorFileSection(
            filePath: filePath,
            cacheKey: cacheKey,
            rows: rows,
            isCollapsed: false,
            additions: 0,
            deletions: 0,
            isStaged: false
        )
        self.sections = [section]
        self.projectPath = projectPath
        self.cacheKey = cacheKey
        self.mode = mode
        self.wordWrap = wordWrap
        self.fontSize = fontSize
        scrollTargetCacheKey = nil
        scrollRequestVersion = 0
        _unifiedState = State(initialValue: Self.makeState(projectPath: projectPath))
        _leftState = State(initialValue: Self.makeState(projectPath: projectPath))
        _rightState = State(initialValue: Self.makeState(projectPath: projectPath))
    }

    init(
        sections: [DiffEditorFileSection],
        projectPath: String,
        cacheKey: String,
        mode: VCSTabState.ViewMode,
        wordWrap: Bool,
        fontSize: CGFloat,
        scrollTargetCacheKey: String?,
        scrollRequestVersion: Int
    ) {
        self.sections = sections
        self.projectPath = projectPath
        self.cacheKey = cacheKey
        self.mode = mode
        self.wordWrap = wordWrap
        self.fontSize = fontSize
        self.scrollTargetCacheKey = scrollTargetCacheKey
        self.scrollRequestVersion = scrollRequestVersion
        _unifiedState = State(initialValue: Self.makeState(projectPath: projectPath))
        _leftState = State(initialValue: Self.makeState(projectPath: projectPath))
        _rightState = State(initialValue: Self.makeState(projectPath: projectPath))
    }

    var body: some View {
        Group {
            switch mode {
            case .unified:
                let document = DiffEditorDocument.unified(sections: sections)
                editor(state: unifiedState, document: document, scrollY: nil)
            case .split:
                let left = DiffEditorDocument.splitLeft(sections: sections)
                let right = DiffEditorDocument.splitRight(sections: sections)
                HStack(spacing: 0) {
                    editor(state: leftState, document: left, scrollY: $splitScrollY)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    Rectangle().fill(MuxyTheme.border).frame(width: 1)
                    editor(state: rightState, document: right, scrollY: $splitScrollY)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
        }
        .background(MuxyTheme.bg)
        .onAppear(perform: syncDocument)
        .onChange(of: signature) { _, _ in syncDocument() }
        .onReceive(NotificationCenter.default.publisher(for: .themeDidChange)) { _ in
            themeRevision &+= 1
        }
    }

    private static func makeState(projectPath: String) -> EditorTabState {
        EditorTabState(projectPath: projectPath, filePath: "Git Diff", readOnlyText: "", diffLineKinds: [])
    }

    private var signature: DiffEditorSignature {
        DiffEditorSignature(cacheKey: cacheKey, mode: mode, rows: sections.map { section in
            "\(section.cacheKey):\(section.rows.count):\(section.rows.first?.id.uuidString ?? ""):\(section.rows.last?.id.uuidString ?? "")"
        }.joined(separator: "|"))
    }

    private func syncDocument() {
        switch mode {
        case .unified:
            apply(DiffEditorDocument.unified(sections: sections), to: unifiedState)
        case .split:
            apply(DiffEditorDocument.splitLeft(sections: sections), to: leftState)
            apply(DiffEditorDocument.splitRight(sections: sections), to: rightState)
        }
    }

    private func apply(_ document: DiffEditorDocument, to state: EditorTabState) {
        state.replaceReadOnlyText(
            document.text,
            filePath: "Git Diff",
            diffLineKinds: document.lineKinds,
            diffGutterLines: document.gutterLines
        )
    }

    private func editor(state: EditorTabState, document: DiffEditorDocument, scrollY: Binding<CGFloat>?) -> some View {
        CodeEditorView(
            state: state,
            editorSettings: editorSettings,
            fontFamilyOverride: "SF Mono",
            fontSizeOverride: fontSize,
            showLineNumbers: false,
            lineWrapping: wordWrap,
            themeVersion: GhosttyService.shared.configVersion + themeRevision,
            showsVerticalScroller: true,
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
            synchronizedScrollY: scrollY,
            scrollToLine: scrollTargetCacheKey.flatMap { document.fileLineIndexes[$0] },
            scrollToLineVersion: scrollRequestVersion,
            onFocus: {}
        )
    }
}

private struct DiffEditorSignature: Equatable {
    let cacheKey: String
    let mode: VCSTabState.ViewMode
    let rows: String
}
