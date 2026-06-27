import SwiftUI

struct SidebarActionButton: View {
    let symbol: String
    let label: String
    var isActive = false
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                .foregroundStyle(foreground)
                .frame(width: TabFocusedSidebarMetrics.controlSlot, height: TabFocusedSidebarMetrics.controlSlot)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(label)
        .accessibilityLabel(label)
        .accessibilityAddTraits(isActive ? .isSelected : [])
    }

    private var foreground: Color {
        if isActive { return MuxyTheme.accent }
        return hovered ? MuxyTheme.fg : MuxyTheme.fgMuted
    }
}
