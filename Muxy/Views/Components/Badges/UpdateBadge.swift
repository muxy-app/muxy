import SwiftUI

struct UpdateBadge: View {
    let version: String
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: UIMetrics.spacing2) {
                Image(systemName: "arrow.down.circle.fill")
                    .font(.system(size: UIMetrics.fontXS, weight: .bold))
                Text("Update \(version)")
                    .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                    .lineLimit(1)
            }
            .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.scaled(3))
            .background(
                RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                    .fill(MuxyTheme.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                    .stroke(MuxyTheme.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("Update available: version \(version)")
        .accessibilityHint("Activates to check for updates")
    }
}
