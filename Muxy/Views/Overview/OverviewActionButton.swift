import SwiftUI

struct OverviewActionButton: View {
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
                .frame(width: OverviewSidebarLayout.controlSlot, height: OverviewSidebarLayout.controlSlot)
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

struct OverviewRow<Leading: View, Trailing: View>: View {
    let highlighted: Bool
    let showsDot: Bool
    let onTap: () -> Void
    @ViewBuilder let leading: () -> Leading
    let title: String
    @ViewBuilder let trailing: () -> Trailing

    @State private var hovered = false

    init(
        title: String,
        highlighted: Bool,
        showsDot: Bool = false,
        onTap: @escaping () -> Void,
        @ViewBuilder leading: @escaping () -> Leading,
        @ViewBuilder trailing: @escaping () -> Trailing = { EmptyView() }
    ) {
        self.title = title
        self.highlighted = highlighted
        self.showsDot = showsDot
        self.onTap = onTap
        self.leading = leading
        self.trailing = trailing
    }

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: UIMetrics.spacing3) {
                leading()
                Text(title)
                    .font(.system(size: UIMetrics.fontEmphasis, weight: .regular))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: UIMetrics.spacing1)
                if showsDot {
                    Circle()
                        .fill(MuxyTheme.accent)
                        .frame(width: UIMetrics.scaled(6), height: UIMetrics.scaled(6))
                }
                trailing()
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.scaled(7))
            .background(background, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityAddTraits((highlighted || showsDot) ? [.isButton, .isSelected] : .isButton)
    }

    private var background: AnyShapeStyle {
        if highlighted { return AnyShapeStyle(MuxyTheme.surface) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }
}
