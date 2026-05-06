import SwiftUI

struct FileTreeIconButton: View {
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: "sidebar.left")
                .font(.system(size: UIMetrics.fontEmphasis, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityLabel("File Tree")
    }
}
