import SwiftUI

struct OverviewActionButton: View {
    let symbol: String
    let label: String
    let action: () -> Void

    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.fontCaption, weight: .semibold))
                .foregroundStyle(hovered ? MuxyTheme.fg : MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(label)
        .accessibilityLabel(label)
    }
}

struct OverviewRow<Leading: View, Trailing: View>: View {
    let isSelected: Bool
    let onTap: () -> Void
    @ViewBuilder let leading: () -> Leading
    let title: String
    @ViewBuilder let trailing: () -> Trailing

    @State private var hovered = false

    init(
        title: String,
        isSelected: Bool,
        onTap: @escaping () -> Void,
        @ViewBuilder leading: @escaping () -> Leading,
        @ViewBuilder trailing: @escaping () -> Trailing = { EmptyView() }
    ) {
        self.title = title
        self.isSelected = isSelected
        self.onTap = onTap
        self.leading = leading
        self.trailing = trailing
    }

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: UIMetrics.spacing3) {
                leading()
                Text(title)
                    .font(.system(size: UIMetrics.fontBody, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer(minLength: UIMetrics.spacing1)
                trailing()
            }
            .padding(.horizontal, UIMetrics.spacing3)
            .padding(.vertical, UIMetrics.scaled(6))
            .background(background, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .accessibilityAddTraits(isSelected ? [.isButton, .isSelected] : .isButton)
    }

    private var background: AnyShapeStyle {
        if isSelected { return AnyShapeStyle(MuxyTheme.accentSoft) }
        if hovered { return AnyShapeStyle(MuxyTheme.hover) }
        return AnyShapeStyle(Color.clear)
    }
}
