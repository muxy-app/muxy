import SwiftUI

extension View {
    func sidebarCardStyle() -> some View {
        padding(UIMetrics.spacing6)
            .background(MuxyTheme.surface, in: RoundedRectangle(cornerRadius: UIMetrics.radiusLG))
            .overlay {
                RoundedRectangle(cornerRadius: UIMetrics.radiusLG)
                    .stroke(MuxyTheme.border, lineWidth: 1)
            }
            .padding(.horizontal, UIMetrics.spacing4)
            .padding(.bottom, UIMetrics.spacing3)
    }

    func sidebarCardPopoverStyle() -> some View {
        padding(UIMetrics.spacing7)
            .frame(width: UIMetrics.scaled(300))
            .background(MuxyTheme.bg)
    }
}

struct SidebarCardHeader: View {
    let symbol: String
    let title: LocalizedStringResource
    let closeLabel: String
    let onClose: () -> Void

    var body: some View {
        HStack(spacing: UIMetrics.spacing3) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.iconXS, weight: .semibold))
                .foregroundStyle(MuxyTheme.accent)
                .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                .background(MuxyTheme.accentSoft, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))

            Text(title)
                .font(.system(size: UIMetrics.fontCaption, weight: .bold))
                .tracking(UIMetrics.scaled(0.7))
                .foregroundStyle(MuxyTheme.accent)
                .textCase(.uppercase)

            Spacer(minLength: UIMetrics.spacing2)

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: UIMetrics.iconXS, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgDim)
                    .frame(width: UIMetrics.controlSmall, height: UIMetrics.controlSmall)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(closeLabel)
            .help(closeLabel)
        }
    }
}
