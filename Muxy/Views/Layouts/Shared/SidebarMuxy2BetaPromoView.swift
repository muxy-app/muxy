import SwiftUI

struct SidebarMuxy2BetaPromoCard: View {
    let onDismiss: () -> Void

    var body: some View {
        SidebarMuxy2BetaPromoContent(onDismiss: onDismiss)
            .sidebarCardStyle()
    }
}

struct SidebarMuxy2BetaPromoPopover: View {
    let onDismiss: () -> Void

    var body: some View {
        SidebarMuxy2BetaPromoContent(onDismiss: onDismiss)
            .sidebarCardPopoverStyle()
    }
}

private struct SidebarMuxy2BetaPromoContent: View {
    let onDismiss: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
            SidebarCardHeader(
                symbol: "sparkles",
                title: L10n.resource("Muxy 2 Beta"),
                closeLabel: L10n.string("Dismiss Muxy 2 Beta"),
                onClose: onDismiss
            )

            Text(L10n.resource("""
            Muxy 2 is rewritten in Rust. It is faster, more efficient, and a true multiplexer. \
            It installs as a separate app next to this one.
            """))
            .font(.system(size: UIMetrics.fontBody))
            .foregroundStyle(MuxyTheme.fg)
            .fixedSize(horizontal: false, vertical: true)

            downloadLink
        }
        .accessibilityElement(children: .contain)
    }

    private var downloadLink: some View {
        Link(destination: HelpLinks.v2BetaReleasesURL) {
            HStack(spacing: UIMetrics.spacing2) {
                Text(L10n.resource("Download Muxy 2 Beta"))
                Image(systemName: "arrow.up.right")
                    .font(.system(size: UIMetrics.iconXS, weight: .semibold))
            }
            .font(.system(size: UIMetrics.fontBody, weight: .medium))
            .foregroundStyle(MuxyTheme.accentForeground)
            .frame(maxWidth: .infinity)
            .frame(height: UIMetrics.controlMedium)
            .background(MuxyTheme.accent, in: RoundedRectangle(cornerRadius: UIMetrics.radiusMD))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(L10n.string("Download Muxy 2 Beta"))
    }
}
