import SwiftUI

struct EditorPane: View {
    @Bindable var state: EditorTabState
    let focused: Bool
    let onFocus: () -> Void
    @Environment(GhosttyService.self) private var ghostty

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
                        LineNumberColumn(lineCount: state.content.components(separatedBy: "\n").count)
                        Rectangle().fill(MuxyTheme.border).frame(width: 1)
                        CodeEditorView(
                            state: state,
                            themeVersion: ghostty.configVersion,
                            searchNeedle: state.searchNeedle,
                            searchNavigationVersion: state.searchNavigationVersion,
                            searchNavigationDirection: state.searchNavigationDirection
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

private struct LineNumberColumn: View {
    let lineCount: Int

    private var gutterWidth: CGFloat {
        CGFloat(max(2, String(max(1, lineCount)).count)) * 9 + 16
    }

    private var lineNumbersText: String {
        let maxLine = max(1, lineCount)
        return (1 ... maxLine).map(String.init).joined(separator: "\n")
    }

    var body: some View {
        VStack(spacing: 0) {
            Text(lineNumbersText)
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgDim)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .padding(.top, 4)
                .padding(.trailing, 8)
                .padding(.leading, 4)
            Spacer(minLength: 0)
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
