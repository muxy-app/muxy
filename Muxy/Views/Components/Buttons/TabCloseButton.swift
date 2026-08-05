import SwiftUI

struct TabCloseButton: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Image(systemName: "xmark")
            .font(.system(size: UIMetrics.fontCaption, weight: .bold))
            .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
            .frame(width: UIMetrics.iconMD, height: UIMetrics.iconMD)
            .background {
                RoundedRectangle(cornerRadius: UIMetrics.radiusSM, style: .continuous)
                    .fill(hovered ? MuxyTheme.hover : Color.clear)
            }
            .contentShape(Rectangle())
            .onHover { hovered = $0 }
            .overlay {
                LeftClickView(action: action)
                    .accessibilityHidden(true)
            }
            .accessibilityLabel(L10n.string("Close Tab"))
            .accessibilityAddTraits(.isButton)
            .accessibilityAction { action() }
    }
}
