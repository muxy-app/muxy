import SwiftUI

struct EditorPane: View {
    @Bindable var state: EditorTabState
    let focused: Bool
    let onFocus: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            EditorBreadcrumb(state: state)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            if state.isLoading {
                Spacer()
                ProgressView()
                    .controlSize(.small)
                Spacer()
            } else if let error = state.errorMessage {
                Spacer()
                Text(error)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.diffRemoveFg)
                Spacer()
            } else {
                CodeEditorView(state: state)
            }
        }
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
        .frame(height: 28)
        .background(MuxyTheme.bg)
    }
}
