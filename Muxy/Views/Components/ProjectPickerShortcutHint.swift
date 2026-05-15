import SwiftUI

struct ProjectPickerShortcutHint: View {
    let keycap: ProjectPickerShortcutKeycap
    let label: String

    var body: some View {
        HStack(spacing: UIMetrics.scaled(4)) {
            HStack(spacing: UIMetrics.scaled(3)) {
                ForEach(Array(keycap.parts.enumerated()), id: \.offset) { _, part in
                    keycapPart(part)
                }
            }
            .lineLimit(1)
            .fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, UIMetrics.scaled(4))
            .padding(.vertical, UIMetrics.scaled(2))
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
            .overlay(RoundedRectangle(cornerRadius: UIMetrics.radiusSM).stroke(MuxyTheme.border, lineWidth: 1))
            Text(label)
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

struct ProjectPickerShortcutKeycap: Hashable {
    let parts: [ProjectPickerShortcutKeycapPart]

    static let navigate = ProjectPickerShortcutKeycap(parts: [.symbol("arrow.up"), .symbol("arrow.down")])
    static let returnKey = ProjectPickerShortcutKeycap(parts: [.text("Return")])
    static let commandReturn = ProjectPickerShortcutKeycap(parts: [.symbol("command"), .text("Return")])
    static let escape = ProjectPickerShortcutKeycap(parts: [.text("Esc")])
    static let optionDelete = ProjectPickerShortcutKeycap(parts: [.symbol("option"), .symbol("delete.left")])
}

enum ProjectPickerShortcutKeycapPart: Hashable {
    case symbol(String)
    case text(String)
}
