import SwiftUI

struct IconButton: View {
    let symbol: String
    var size: CGFloat = 13
    var color: Color = MuxyTheme.fgMuted
    var hoverColor: Color = MuxyTheme.fg
    let accessibilityLabel: String
    let action: () -> Void

    var body: some View {
        IconButtonChrome(
            color: color,
            hoverColor: hoverColor,
            accessibilityLabel: accessibilityLabel,
            action: action
        ) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.scaled(size), weight: .semibold))
        }
    }
}

struct IconButtonChrome<Label: View>: View {
    var color: Color = MuxyTheme.fgMuted
    var hoverColor: Color = MuxyTheme.fg
    let accessibilityLabel: String
    let action: () -> Void
    @ViewBuilder var label: Label
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            label
                .foregroundStyle(hovered ? hoverColor : color)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel(accessibilityLabel)
    }
}
