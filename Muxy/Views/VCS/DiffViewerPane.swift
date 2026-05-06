import SwiftUI

struct DiffViewerPane: View {
    @Bindable var state: DiffViewerTabState
    let focused: Bool
    let onFocus: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            DiffViewerBreadcrumb(state: state)
            Rectangle().fill(MuxyTheme.border).frame(height: 1)
            ScrollView([.vertical]) {
                DiffBodyView(
                    isLoading: state.vcs.diffCache.isLoading(state.filePath),
                    error: state.vcs.diffCache.error(for: state.filePath),
                    diff: state.vcs.diffCache.diff(for: state.filePath),
                    filePath: state.filePath,
                    mode: state.mode,
                    onLoadFull: { state.refresh(forceFull: true) },
                    suppressLeadingTopBorder: true
                )
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .background(MuxyTheme.bg)
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { onFocus() })
    }
}

private struct DiffViewerBreadcrumb: View {
    @Bindable var state: DiffViewerTabState

    private var loadedDiff: DiffCache.LoadedDiff? {
        state.vcs.diffCache.diff(for: state.filePath)
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            FileDiffIcon()
                .stroke(MuxyTheme.fgDim, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                .frame(width: UIMetrics.scaled(11), height: UIMetrics.scaled(11))

            Text(state.filePath)
                .font(.system(size: UIMetrics.fontFootnote))
                .foregroundStyle(MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.middle)
                .textSelection(.enabled)

            if state.isStaged {
                Text("Staged")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .padding(.horizontal, UIMetrics.scaled(5))
                    .padding(.vertical, UIMetrics.scaled(1))
                    .background(MuxyTheme.surface, in: Capsule())
            }

            if let diff = loadedDiff {
                if diff.additions > 0 {
                    Text("+\(diff.additions)")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.diffAddFg)
                }
                if diff.deletions > 0 {
                    Text("-\(diff.deletions)")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .semibold, design: .monospaced))
                        .foregroundStyle(MuxyTheme.diffRemoveFg)
                }
            }

            Spacer()

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
