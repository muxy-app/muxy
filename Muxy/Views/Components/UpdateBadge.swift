import SwiftUI

struct UpdateBadge: View {
    let version: String
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: "arrow.down.circle.fill")
                    .font(.system(size: 9, weight: .bold))
                Text("Update \(version)")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .lineLimit(1)
            }
            .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(MuxyTheme.surface)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 5)
                    .stroke(MuxyTheme.border, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("Update available: version \(version)")
        .accessibilityHint("Activates to check for updates")
    }
}
