import SwiftUI

struct ShortcutBadge: View {
    let label: String
    var compact: Bool = false

    var body: some View {
        Text(label)
            .font(.system(size: compact ? UIMetrics.fontXS : UIMetrics.fontFootnote, weight: .medium, design: .rounded))
            .foregroundStyle(.white)
            .padding(.horizontal, compact ? UIMetrics.spacing2 : UIMetrics.spacing3)
            .padding(.vertical, compact ? UIMetrics.scaled(1) : UIMetrics.scaled(3))
            .background(.ultraThinMaterial, in: Capsule())
            .overlay(Capsule().strokeBorder(.white.opacity(0.15), lineWidth: 0.5))
            .shadow(
                color: .black.opacity(0.25),
                radius: UIMetrics.scaled(compact ? 2 : 4),
                y: UIMetrics.scaled(compact ? 1 : 2)
            )
            .accessibilityLabel("Keyboard shortcut: \(label)")
    }
}
