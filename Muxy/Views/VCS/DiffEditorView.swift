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
    @State private var documentRevision = 0
    @State private var splitScrollY: CGFloat = 0
    @State private var unifiedState: EditorTabState
    @State private var leftState: EditorTabState
    @State private var rightState: EditorTabState
    @State private var syncTask: Task<Void, Never>?
    @State private var appliedSignature: DiffEditorSignature?

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
        let targetSignature = signature
        guard appliedSignature != targetSignature else { return }
        syncTask?.cancel()
        if rows.count <= Self.synchronousRowThreshold {
            applyAll(DiffDocumentBuilder.build(rows: rows, mode: mode, options: renderOptions))
            appliedSignature = targetSignature
            syncTask = nil
            return
        }
        let rows = rows
        let mode = mode
        let options = renderOptions
        syncTask = Task {
            let documents = await GitProcessRunner.offMain {
                DiffDocumentBuilder.build(rows: rows, mode: mode, options: options)
            }
            guard !Task.isCancelled else { return }
            await MainActor.run {
                guard signature == targetSignature else { return }
                applyAll(documents)
                appliedSignature = targetSignature
            }
        }
    }

    private var renderOptions: DiffEditorDocument.RenderOptions {
        DiffEditorDocument.RenderOptions(maxLineCharacters: maxLineCharacters)
    }

    private func applyAll(_ documents: DocumentSet) {
        if let unified = documents.unified {
            apply(unified, to: unifiedState)
        }
        if let left = documents.left {
            apply(left, to: leftState)
        }
        if let right = documents.right {
            apply(right, to: rightState)
        }
    }

    private func apply(_ document: DiffEditorDocument, to state: EditorTabState) {
        state.replaceReadOnlyText(
            document.text,
            filePath: filePath,
            diffLineKinds: document.lineKinds,
            diffGutterLines: document.gutterLines
        )
        documentRevision &+= 1
    }

    private static let synchronousRowThreshold = 500

    private func editor(state: EditorTabState, scrollY: Binding<CGFloat>?) -> some View {
        CodeEditorView(
            state: state,
            editorSettings: editorSettings,
            fontFamilyOverride: "SF Mono",
            fontSizeOverride: fontSize,
            showLineNumbers: false,
            lineWrapping: wordWrap,
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

private struct DocumentSet {
    let unified: DiffEditorDocument?
    let left: DiffEditorDocument?
    let right: DiffEditorDocument?
}

private enum DiffDocumentBuilder {
    static func build(
        rows: [DiffDisplayRow],
        mode: VCSTabState.ViewMode,
        options: DiffEditorDocument.RenderOptions
    ) -> DocumentSet {
        switch mode {
        case .unified:
            DocumentSet(unified: DiffEditorDocument.unified(rows: rows, options: options), left: nil, right: nil)
        case .split:
            DocumentSet(
                unified: nil,
                left: DiffEditorDocument.splitLeft(rows: rows, options: options),
                right: DiffEditorDocument.splitRight(rows: rows, options: options)
            )
        }
    }
}
