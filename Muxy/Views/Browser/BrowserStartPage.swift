import SwiftUI

struct BrowserStartPage: View {
    let searchEngineName: String
    let onFocusAddress: () -> Void

    var body: some View {
        VStack(spacing: UIMetrics.spacing7) {
            Spacer()
            Image(systemName: "globe")
                .font(.system(size: UIMetrics.fontMega, weight: .light))
                .foregroundStyle(MuxyTheme.fgMuted)
            Text("New Tab")
                .font(.system(size: UIMetrics.fontHeadline, weight: .semibold))
                .foregroundStyle(MuxyTheme.fg)
            Text("Search with \(searchEngineName) or enter a website address.")
                .font(.system(size: UIMetrics.fontBody))
                .foregroundStyle(MuxyTheme.fgMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: UIMetrics.scaled(360))
            Button(action: onFocusAddress) {
                HStack(spacing: UIMetrics.spacing3) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                    Text("Search or enter address")
                        .font(.system(size: UIMetrics.fontBody))
                    Spacer(minLength: 0)
                    Text("⌘L")
                        .font(.system(size: UIMetrics.fontFootnote, weight: .medium, design: .rounded))
                        .foregroundStyle(MuxyTheme.fgDim)
                }
                .foregroundStyle(MuxyTheme.fgMuted)
                .padding(.horizontal, UIMetrics.spacing4)
                .frame(width: UIMetrics.scaled(360), height: UIMetrics.controlLarge)
                .background(MuxyTheme.surface)
                .clipShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
                .overlay(
                    RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                        .strokeBorder(MuxyTheme.border, lineWidth: 1)
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(MuxyTheme.bg)
    }
}
