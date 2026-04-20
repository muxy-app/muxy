import SwiftUI

struct DiffBodyView: View {
    let isLoading: Bool
    let error: String?
    let diff: DiffCache.LoadedDiff?
    let filePath: String
    let mode: VCSTabState.ViewMode
    let onLoadFull: (() -> Void)?
    var suppressLeadingTopBorder: Bool = false

    var body: some View {
        Group {
            if isLoading, diff == nil {
                ProgressView()
                    .frame(maxWidth: .infinity)
                    .padding(14)
            } else if let error {
                Text(error)
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
            } else if let diff {
                VStack(spacing: 0) {
                    if diff.truncated, let onLoadFull {
                        truncatedBanner(onLoadFull: onLoadFull)
                        Rectangle().fill(MuxyTheme.border).frame(height: 1)
                    }

                    switch mode {
                    case .unified:
                        UnifiedDiffView(
                            rows: diff.rows,
                            filePath: filePath,
                            suppressLeadingTopBorder: suppressLeadingTopBorder && !diff.truncated
                        )
                    case .split:
                        SplitDiffView(
                            rows: diff.rows,
                            filePath: filePath,
                            suppressLeadingTopBorder: suppressLeadingTopBorder && !diff.truncated
                        )
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                Text("No diff output")
                    .font(.system(size: 12))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(12)
            }
        }
        .background(MuxyTheme.bg)
    }

    private func truncatedBanner(onLoadFull: @escaping () -> Void) -> some View {
        HStack {
            Text("Large diff preview")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(MuxyTheme.fgMuted)
            Spacer(minLength: 0)
            Button("Load full diff", action: onLoadFull)
                .buttonStyle(.plain)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
    }
}
