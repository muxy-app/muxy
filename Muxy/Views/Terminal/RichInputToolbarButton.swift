import SwiftUI

struct RichInputToolbarButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(.horizontal, 6)
            .padding(.vertical, 4)
            .foregroundStyle(MuxyTheme.fgMuted)
            .background(configuration.isPressed ? MuxyTheme.surface : .clear)
            .clipShape(RoundedRectangle(cornerRadius: 5))
            .contentShape(Rectangle())
    }
}
