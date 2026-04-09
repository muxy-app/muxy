import SwiftUI

struct EditorPane: View {
    @Bindable var state: EditorTabState
    let focused: Bool
    let onFocus: () -> Void
    @Environment(GhosttyService.self) private var ghostty
    @State private var editorSettings = EditorSettings.shared
    @State private var lineLayouts: [LineLayoutInfo] = []

    var body: some View {
        VStack(spacing: 0) {
            EditorBreadcrumb(state: state)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            if state.isLoading {
                loadingView
            } else if let error = state.errorMessage {
                errorView(error)
            } else {
                ZStack(alignment: .topTrailing) {
                    HStack(spacing: 0) {
                        if editorSettings.showLineNumbers {
                            LineNumberGutter(
                                layouts: lineLayouts,
                                fontSize: editorSettings.fontSize,
                                fontFamily: editorSettings.fontFamily,
                                activeLine: state.cursorLine
                            )
                            Rectangle().fill(MuxyTheme.border).frame(width: 1)
                        }
                        CodeEditorView(
                            state: state,
                            editorSettings: editorSettings,
                            themeVersion: ghostty.configVersion,
                            searchNeedle: state.searchNeedle,
                            searchNavigationVersion: state.searchNavigationVersion,
                            searchNavigationDirection: state.searchNavigationDirection,
                            onLineLayoutChange: { layouts in
                                lineLayouts = layouts
                            }
                        )
                    }

                    if state.searchVisible {
                        EditorSearchBar(
                            state: state,
                            onNext: {
                                state.navigateSearch(.next)
                            },
                            onPrevious: {
                                state.navigateSearch(.previous)
                            },
                            onClose: {
                                state.searchVisible = false
                                state.searchNeedle = ""
                                state.searchMatchCount = 0
                                state.searchCurrentIndex = 0
                            }
                        )
                    }
                }
            }
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .onTapGesture(perform: onFocus)
        .onReceive(NotificationCenter.default.publisher(for: .findInTerminal)) { _ in
            guard focused else { return }
            state.searchVisible = true
        }
    }

    private var loadingView: some View {
        VStack {
            Spacer()
            ProgressView().controlSize(.small)
            Spacer()
        }
    }

    private func errorView(_ error: String) -> some View {
        VStack {
            Spacer()
            Text(error)
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.diffRemoveFg)
            Spacer()
        }
    }
}

private struct LineNumberGutter: View {
    let layouts: [LineLayoutInfo]
    let fontSize: CGFloat
    let fontFamily: String
    let activeLine: Int

    private var gutterFontSize: CGFloat {
        max(9, fontSize - 2)
    }

    private var gutterWidth: CGFloat {
        let maxLine = layouts.last?.lineNumber ?? 1
        let charWidth = gutterFontSize * 0.65
        return CGFloat(max(2, String(maxLine).count)) * charWidth + 16
    }

    var body: some View {
        Canvas { context, size in
            let font = Font.custom(fontFamily, size: gutterFontSize)
            let dimColor = Color(MuxyTheme.fgDim)
            let activeColor = Color(MuxyTheme.fgMuted)
            for layout in layouts {
                let isActive = layout.lineNumber == activeLine
                let text = Text(verbatim: "\(layout.lineNumber)")
                    .font(font)
                    .foregroundStyle(isActive ? activeColor : dimColor)
                let resolved = context.resolve(text)
                let textSize = resolved.measure(in: size)
                let x = size.width - textSize.width - 8
                let y = layout.yOffset + (layout.height - textSize.height) / 2
                context.draw(resolved, at: CGPoint(x: x, y: y), anchor: .topLeading)
            }
        }
        .frame(width: gutterWidth)
        .background(MuxyTheme.bg)
    }
}

private struct EditorBreadcrumb: View {
    let state: EditorTabState

    private var relativePath: String {
        let full = state.filePath
        let base = state.projectPath
        guard full.hasPrefix(base) else { return state.fileName }
        var rel = String(full.dropFirst(base.count))
        if rel.hasPrefix("/") { rel = String(rel.dropFirst()) }
        return rel
    }

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: "doc.text")
                .font(.system(size: 10))
                .foregroundStyle(MuxyTheme.fgDim)
            Text(relativePath)
                .font(.system(size: 11))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)
            if state.isModified {
                Circle()
                    .fill(MuxyTheme.fg)
                    .frame(width: 6, height: 6)
            }
            Spacer()
            Text("Ln \(state.cursorLine), Col \(state.cursorColumn)")
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
        }
        .padding(.horizontal, 10)
        .frame(height: 32)
        .background(MuxyTheme.bg)
    }
}
