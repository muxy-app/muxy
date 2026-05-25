import SwiftUI

struct SingleDiffEditorView: View {
    let rows: [DiffDisplayRow]
    let projectPath: String
    let filePath: String
    let cacheKey: String
    let mode: VCSTabState.ViewMode
    let wordWrap: Bool
    let fontSize: CGFloat
    let maxLineCharacters: Int?
    let externalScrollY: CGFloat?
    let passesScrollWheelToParent: Bool

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
        fontSize: CGFloat,
        maxLineCharacters: Int? = nil,
        externalScrollY: CGFloat? = nil,
        passesScrollWheelToParent: Bool = false
    ) {
        self.rows = rows
        self.projectPath = projectPath
        self.filePath = filePath
        self.cacheKey = cacheKey
        self.mode = mode
        self.wordWrap = wordWrap
        self.fontSize = fontSize
        self.maxLineCharacters = maxLineCharacters
        self.externalScrollY = externalScrollY
        self.passesScrollWheelToParent = passesScrollWheelToParent
        _unifiedState = State(initialValue: Self.makeState(projectPath: projectPath, filePath: filePath))
        _leftState = State(initialValue: Self.makeState(projectPath: projectPath, filePath: filePath))
        _rightState = State(initialValue: Self.makeState(projectPath: projectPath, filePath: filePath))
    }

    var body: some View {
        Group {
            switch mode {
            case .unified:
                editor(state: unifiedState, scrollY: externalScrollBinding)
            case .split:
                HStack(spacing: 0) {
                    editor(state: leftState, scrollY: externalScrollBinding ?? $splitScrollY)
                    Rectangle().fill(MuxyTheme.border).frame(width: 1)
                    editor(state: rightState, scrollY: externalScrollBinding ?? $splitScrollY)
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

    private static func makeState(projectPath: String, filePath: String) -> EditorTabState {
        EditorTabState(projectPath: projectPath, filePath: filePath, readOnlyText: "", diffLineKinds: [])
    }

    private var signature: DiffEditorSignature {
        DiffEditorSignature(
            cacheKey: cacheKey,
            mode: mode,
            rows: "\(rows.count):\(rows.first?.id.uuidString ?? ""):\(rows.last?.id.uuidString ?? ""):\(maxLineCharacters ?? 0)"
        )
    }

    private func syncDocument() {
        switch mode {
        case .unified:
            apply(DiffEditorDocument.unified(rows: rows, options: renderOptions), to: unifiedState)
        case .split:
            apply(DiffEditorDocument.splitLeft(rows: rows, options: renderOptions), to: leftState)
            apply(DiffEditorDocument.splitRight(rows: rows, options: renderOptions), to: rightState)
        }
    }

    private var renderOptions: DiffEditorDocument.RenderOptions {
        DiffEditorDocument.RenderOptions(maxLineCharacters: maxLineCharacters)
    }

    private func apply(_ document: DiffEditorDocument, to state: EditorTabState) {
        state.replaceReadOnlyText(
            document.text,
            filePath: filePath,
            diffLineKinds: document.lineKinds,
            diffGutterLines: document.gutterLines
        )
    }

    private func editor(state: EditorTabState, scrollY: Binding<CGFloat>?) -> some View {
        CodeEditorView(
            state: state,
            editorSettings: editorSettings,
            fontFamilyOverride: "SF Mono",
            fontSizeOverride: fontSize,
            showLineNumbers: false,
            lineWrapping: wordWrap,
            themeVersion: GhosttyService.shared.configVersion + themeRevision,
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
            synchronizedScrollY: scrollY,
            passesScrollWheelToParent: passesScrollWheelToParent,
            onFocus: {}
        )
    }

    private var externalScrollBinding: Binding<CGFloat>? {
        externalScrollY.map { Binding.constant($0) }
    }
}

private struct DiffEditorSignature: Equatable {
    let cacheKey: String
    let mode: VCSTabState.ViewMode
    let rows: String
}
