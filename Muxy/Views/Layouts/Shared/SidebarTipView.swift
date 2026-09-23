import Foundation
import SwiftUI

struct SidebarTipCard: View {
    let store: TipsStore
    let onHide: () -> Void

    var body: some View {
        if store.currentTip != nil {
            SidebarTipContent(store: store, onHide: onHide)
                .sidebarCardStyle()
        }
    }
}

struct SidebarTipPopover: View {
    let store: TipsStore
    let onHide: () -> Void

    var body: some View {
        SidebarTipContent(store: store, onHide: onHide)
            .sidebarCardPopoverStyle()
    }
}

private struct SidebarTipContent: View {
    let store: TipsStore
    let onHide: () -> Void
    @State private var showHideConfirmation = false

    var body: some View {
        if let tip = store.currentTip {
            VStack(alignment: .leading, spacing: UIMetrics.spacing5) {
                SidebarCardHeader(
                    symbol: "lightbulb.fill",
                    title: L10n.resource("Muxy Tip"),
                    closeLabel: L10n.string("Hide Tips")
                ) {
                    showHideConfirmation = true
                }

                Text(TipDescriptionPresentation.attributedDescription(tip.description))
                    .font(.system(size: UIMetrics.fontBody))
                    .foregroundStyle(MuxyTheme.fg)
                    .fixedSize(horizontal: false, vertical: true)
                    .tint(MuxyTheme.accent)

                controls
            }
            .accessibilityElement(children: .contain)
            .alert(L10n.string("Hide Tips?"), isPresented: $showHideConfirmation) {
                Button(L10n.string("Hide Tips"), role: .destructive, action: onHide)
                    .keyboardShortcut(.defaultAction)
                Button(L10n.string("Cancel"), role: .cancel) {}
                    .keyboardShortcut(.cancelAction)
            } message: {
                Text(L10n.resource(
                    "You can show tips again in Settings → Interface → Sidebar by turning on Show Tips."
                ))
            }
        }
    }

    private var controls: some View {
        HStack(spacing: UIMetrics.spacing2) {
            Text(L10n.resource("\(store.position) of \(store.tips.count)"))
                .font(.system(size: UIMetrics.fontCaption).monospacedDigit())
                .foregroundStyle(MuxyTheme.fgDim)

            Spacer(minLength: UIMetrics.spacing3)

            navigationButton(
                symbol: "chevron.left",
                label: L10n.string("Previous Tip"),
                action: store.showPrevious
            )
            navigationButton(
                symbol: "chevron.right",
                label: L10n.string("Next Tip"),
                action: store.showNext
            )
        }
    }

    private func navigationButton(
        symbol: String,
        label: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: UIMetrics.iconXS, weight: .semibold))
                .foregroundStyle(MuxyTheme.fgMuted)
                .frame(width: UIMetrics.controlMedium, height: UIMetrics.controlSmall)
                .background(MuxyTheme.hover, in: RoundedRectangle(cornerRadius: UIMetrics.radiusSM))
                .overlay {
                    RoundedRectangle(cornerRadius: UIMetrics.radiusSM)
                        .stroke(MuxyTheme.border, lineWidth: 1)
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .help(label)
    }
}

@MainActor
enum TipDescriptionPresentation {
    static func attributedDescription(
        _ description: String,
        localization: LocalizationService = .shared
    ) -> AttributedString {
        let localizedDescription = localization.string(
            LocalizedStringResource(String.LocalizationValue(description))
        )
        return (try? AttributedString(
            markdown: localizedDescription,
            options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        )) ?? AttributedString(localizedDescription)
    }
}
