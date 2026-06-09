import SwiftUI

struct IconButton: View {
    let symbol: String
    var size: CGFloat = 13
    var color: Color = MuxyTheme.fgMuted
    var hoverColor: Color = MuxyTheme.fg
    var showsBadge = false
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
                .overlay(alignment: .topTrailing) {
                    if showsBadge {
                        IconButtonBadge()
                    }
                }
        }
    }
}

private struct IconButtonBadge: View {
    var body: some View {
        Circle()
            .fill(MuxyTheme.accent)
            .frame(width: UIMetrics.scaled(6), height: UIMetrics.scaled(6))
            .overlay(
                Circle().stroke(MuxyTheme.bg, lineWidth: UIMetrics.scaled(1.5))
            )
            .offset(x: UIMetrics.scaled(4), y: UIMetrics.scaled(-4))
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
