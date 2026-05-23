import SwiftUI

struct DiffBodyView: View {
    let isLoading: Bool
    let error: String?
    let diff: DiffCache.LoadedDiff?
    let projectPath: String
    let filePath: String
    let cacheKey: String
    let mode: VCSTabState.ViewMode
    let wordWrap: Bool
    let fontSize: CGFloat
    let onLoadFull: (() -> Void)?
    var suppressLeadingTopBorder: Bool = false

    var body: some View {
        Group {
            if isLoading, diff == nil {
                ProgressView()
                    .frame(maxWidth: .infinity)
                    .padding(UIMetrics.scaled(14))
            } else if let error {
                Text(error)
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(UIMetrics.spacing6)
            } else if let diff {
                VStack(spacing: 0) {
                    if diff.truncated, let onLoadFull {
                        truncatedBanner(onLoadFull: onLoadFull)
                        Rectangle().fill(MuxyTheme.border).frame(height: 1)
                    }

                    DiffEditorView(
                        rows: diff.rows,
                        projectPath: projectPath,
                        filePath: filePath,
                        cacheKey: cacheKey,
                        mode: mode,
                        wordWrap: wordWrap,
                        fontSize: fontSize
                    )
                    .id(cacheKey)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            } else {
                Text("No diff output")
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(UIMetrics.spacing6)
            }
        }
        .background(MuxyTheme.bg)
    }

    private func truncatedBanner(onLoadFull: @escaping () -> Void) -> some View {
        HStack {
            Text("Large diff preview")
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
            Spacer(minLength: 0)
            Button("Load full diff", action: onLoadFull)
                .buttonStyle(.plain)
                .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
        }
        .padding(.horizontal, UIMetrics.spacing5)
        .padding(.vertical, UIMetrics.spacing4)
    }
}
