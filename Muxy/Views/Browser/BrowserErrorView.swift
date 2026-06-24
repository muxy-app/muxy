import SwiftUI

struct BrowserErrorView: View {
    let error: BrowserLoadError
    let onRetry: () -> Void

    var body: some View {
        VStack(spacing: UIMetrics.spacing6) {
            Image(systemName: "exclamationmark.triangle")
                .font(.system(size: UIMetrics.iconXXL, weight: .regular))
                .foregroundStyle(MuxyTheme.fgMuted)

            VStack(spacing: UIMetrics.spacing3) {
                Text("This page can't be opened")
                    .font(.system(size: UIMetrics.fontTitle, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)

                Text(error.message)
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fgMuted)
                    .multilineTextAlignment(.center)

                if !error.failedURL.isEmpty {
                    Text(error.failedURL)
                        .font(.system(size: UIMetrics.fontFootnote, design: .monospaced))
                        .foregroundStyle(MuxyTheme.fgDim)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            Button(action: onRetry) {
                Text("Try Again")
                    .font(.system(size: UIMetrics.fontBody, weight: .medium))
                    .foregroundStyle(MuxyTheme.accentForeground)
                    .padding(.horizontal, UIMetrics.spacing7)
                    .frame(height: UIMetrics.controlLarge)
                    .background(MuxyTheme.accent)
                    .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            }
            .buttonStyle(.plain)
        }
        .padding(UIMetrics.spacing9)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MuxyTheme.bg)
    }
}
