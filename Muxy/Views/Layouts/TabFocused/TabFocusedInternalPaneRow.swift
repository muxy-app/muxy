import SwiftUI

struct TabFocusedInternalPaneRow: View {
    let project: Project
    let area: TabArea
    let tab: TerminalTab
    let pane: TerminalPaneState
    let active: Bool
    let onFocus: () -> Void

    @Environment(AppState.self) private var appState

    private var rowBackground: AnyShapeStyle {
        if active { return AnyShapeStyle(MuxyTheme.surface) }
        return AnyShapeStyle(Color.clear)
    }

    var body: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Image(systemName: "terminal")
                .font(.system(size: UIMetrics.fontCaption, weight: .medium))
                .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.iconSM, height: UIMetrics.iconSM)

            Text(pane.title)
                .font(.system(size: UIMetrics.fontCaption))
                .foregroundStyle(active ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer(minLength: 0)
        }
        .padding(.leading, TabFocusedSidebarMetrics.tabContentLeadingInset + UIMetrics.spacing3)
        .padding(.trailing, TabFocusedSidebarMetrics.rowHorizontalInset)
        .frame(minHeight: TabFocusedSidebarMetrics.rowHeight - 4)
        .background {
            RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous)
                .fill(rowBackground)
        }
        .padding(.horizontal, TabFocusedSidebarMetrics.rowOuterInset)
        .padding(.vertical, UIMetrics.spacing1)
        .contentShape(RoundedRectangle(cornerRadius: TabFocusedSidebarMetrics.rowCornerRadius, style: .continuous))
        .onTapGesture { onFocus() }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Pane: \(pane.title)")
        .accessibilityAddTraits(active ? [.isButton, .isSelected] : .isButton)
    }
}
