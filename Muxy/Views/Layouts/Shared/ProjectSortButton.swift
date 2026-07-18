import SwiftUI

enum ProjectSortButton {
    struct Label: View {
        let mode: ProjectSortMode
        @State private var hovered = false

        var body: some View {
            Image(systemName: "arrow.up.arrow.down")
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.accent : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlMedium)
                .background(
                    hovered ? MuxyTheme.hover : MuxyTheme.surface,
                    in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD)
                )
                .onHover { hovered = $0 }
                .accessibilityLabel("Sort Projects")
        }
    }
}
