import AppKit
import SwiftUI

struct EditorPane: View {
    @Bindable var state: EditorTabState
    let focused: Bool
    let onFocus: () -> Void
    @Environment(GhosttyService.self) private var ghostty
    @State private var editorSettings = EditorSettings.shared
    @State private var lineLayouts: [LineLayoutInfo] = []
    @State private var totalLineCount: Int = 1

    var body: some View {
        VStack(spacing: 0) {
            EditorBreadcrumb(state: state)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            if state.awaitingLargeFileConfirmation {
                largeFileConfirmation
            } else if state.isLoading {
                loadingView
            } else if let error = state.errorMessage {
                errorView(error)
            } else {
                ZStack(alignment: .topTrailing) {
                    HStack(spacing: 0) {
                        if editorSettings.showLineNumbers {
                            LineNumberGutter(
                                layouts: lineLayouts,
                                totalLineCount: totalLineCount,
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
                            focused: focused,
                            searchNeedle: state.searchNeedle,
                            searchNavigationVersion: state.searchNavigationVersion,
                            searchNavigationDirection: state.searchNavigationDirection,
                            searchCaseSensitive: state.searchCaseSensitive,
                            searchUseRegex: state.searchUseRegex,
                            replaceText: state.replaceText,
                            replaceVersion: state.replaceVersion,
                            replaceAllVersion: state.replaceAllVersion,
                            editorFocusVersion: state.editorFocusVersion,
                            onLineLayoutChange: { layouts in
                                lineLayouts = layouts
                            },
                            onTotalLineCountChange: { count in
                                totalLineCount = count
                            }
                        )
                    }

                    if state.isIncrementalLoading {
                        HStack(spacing: 6) {
                            ProgressView().controlSize(.mini)
                            Text("Loading full file...")
                                .font(.system(size: 11))
                                .foregroundStyle(MuxyTheme.fgMuted)
                        }
                        .padding(.horizontal, 8)
                        .padding(.vertical, 6)
                        .background(MuxyTheme.bg.opacity(0.92))
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .stroke(MuxyTheme.border, lineWidth: 1)
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                        .padding(.top, 6)
                        .padding(.trailing, state.searchVisible ? 260 : 8)
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
                            onReplace: {
                                state.requestReplaceCurrent()
                            },
                            onReplaceAll: {
                                state.requestReplaceAll()
                            },
                            onClose: {
                                state.searchVisible = false
                                state.editorFocusVersion += 1
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
            if !state.currentSelection.isEmpty {
                state.searchNeedle = state.currentSelection
            }
            state.searchVisible = true
            state.searchFocusVersion += 1
        }
    }

    private var loadingView: some View {
        VStack {
            Spacer()
            ProgressView().controlSize(.small)
            Spacer()
        }
    }

    private var largeFileConfirmation: some View {
        VStack(spacing: 16) {
            Spacer()
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: 28))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("Large File")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text("This file is \(formattedLargeFileSize). Large files may slow down the editor.")
                .font(.system(size: 12))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
            HStack(spacing: 8) {
                Button("Cancel") {
                    state.cancelLargeFileOpen()
                }
                .keyboardShortcut(.cancelAction)
                Button("Open Anyway") {
                    state.confirmLargeFileOpen()
                }
                .keyboardShortcut(.defaultAction)
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var formattedLargeFileSize: String {
        let formatter = ByteCountFormatter()
        formatter.allowedUnits = [.useMB, .useKB, .useGB]
        formatter.countStyle = .file
        return formatter.string(fromByteCount: state.largeFileSize)
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
    let totalLineCount: Int
    let fontSize: CGFloat
    let fontFamily: String
    let activeLine: Int

    private var gutterFontSize: CGFloat {
        max(9, fontSize - 2)
    }

    private var gutterWidth: CGFloat {
        let digits = max(4, String(max(1, totalLineCount)).count)
        let sample = String(repeating: "8", count: digits)
        let font = NSFont(name: fontFamily, size: gutterFontSize) ?? .monospacedDigitSystemFont(ofSize: gutterFontSize, weight: .regular)
        let textWidth = (sample as NSString).size(withAttributes: [.font: font]).width
        return ceil(textWidth) + 16
    }

    var body: some View {
        Canvas { context, size in
            let font = Font.custom(fontFamily, size: gutterFontSize)
            let dimColor = Color(MuxyTheme.fgDim)
            let activeColor = Color(MuxyTheme.fgMuted)
            let sampleResolved = context.resolve(Text(verbatim: "0").font(font).foregroundStyle(dimColor))
            let charHeight = sampleResolved.measure(in: size).height

            for layout in layouts {
                let isActive = layout.lineNumber == activeLine
                let resolved = context.resolve(
                    Text(verbatim: "\(layout.lineNumber)")
                        .font(font)
                        .foregroundStyle(isActive ? activeColor : dimColor)
                )
                let textWidth = resolved.measure(in: size).width
                let x = size.width - textWidth - 8
                let y = layout.yOffset + (layout.height - charHeight) / 2
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
            if state.isReadOnly {
                Label("Read-only", systemImage: "lock.fill")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(MuxyTheme.diffHunkFg)
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
