import SwiftUI

struct IconButton: View {
    let symbol: String
    var size: CGFloat = 13
    var color: Color = MuxyTheme.fgMuted
    var hoverColor: Color = MuxyTheme.fg
    let accessibilityLabel: String
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.scaled(size), weight: .semibold))
                .foregroundStyle(hovered ? hoverColor : color)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel(accessibilityLabel)
    }
}
