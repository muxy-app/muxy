import SwiftUI

struct ProjectPickerShortcutHint: View {
    let shortcut: ProjectPickerFooterShortcut

    var body: some View {
        HStack(spacing: UIMetrics.scaled(4)) {
            HStack(spacing: UIMetrics.scaled(3)) {
                ForEach(Array(shortcut.keycap.parts.enumerated()), id: \.offset) { _, part in
                    keycapPart(part)
                }
            }
            .lineLimit(1)
            .fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, UIMetrics.scaled(4))
            .padding(.vertical, UIMetrics.scaled(2))
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            Text(shortcut.label)
                .font(.system(size: UIMetrics.fontFootnote, weight: .medium))
                .foregroundStyle(MuxyTheme.fgDim)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
        }
        .fixedSize(horizontal: true, vertical: false)
    }

    @ViewBuilder
    private func keycapPart(_ part: ProjectPickerShortcutKeycapPart) -> some View {
        switch part {
        case let .symbol(name):
            Image(systemName: name)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
        case let .text(text):
            Text(text)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold, design: .monospaced))
                .foregroundStyle(MuxyTheme.fgMuted)
        }
    }
}
