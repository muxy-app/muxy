import SwiftUI

struct ShortcutIconBadge: View {
    let number: Int
    let size: CGFloat
    let combo: KeyCombo

    var body: some View {
        RoundedRectangle(cornerRadius: size * 0.25)
            .fill(MuxyTheme.accent)
            .frame(width: size, height: size)
            .overlay {
                Text("\(number)")
                    .font(.system(size: size * 0.7, weight: .bold, design: .rounded))
                    .foregroundStyle(MuxyTheme.bg)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
            }
            .accessibilityLabel("Keyboard shortcut: \(combo.displayString)")
    }
}
