import SwiftUI

struct PanelContainer<Content: View>: View {
    let chrome: PanelChrome
    let mode: PanelMode
    let position: PanelPosition
    let onClose: (() -> Void)?
    let onTogglePin: (() -> Void)?
    let onTogglePosition: (() -> Void)?
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(spacing: 0) {
            if chrome.hasHeaderContent {
                header
                Rectangle().fill(MuxyTheme.border).frame(height: 1)
            }
            content()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(MuxyTheme.bg)
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing2) {
            if let symbol = chrome.iconSymbol {
                Image(systemName: symbol)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            if let title = chrome.title {
                Text(title)
                    .font(.system(size: UIMetrics.fontBody, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fg)
                    .lineLimit(1)
            }
            Spacer(minLength: UIMetrics.spacing2)
            HStack(spacing: 0) {
                ForEach(chrome.trailingButtons) { button in
                    customButton(button)
                }
                if chrome.shows(.position), let onTogglePosition {
                    control(
                        symbol: positionSymbol,
                        label: positionLabel,
                        action: onTogglePosition
                    )
                }
                if chrome.shows(.pin), let onTogglePin {
                    control(
                        symbol: mode == .floating ? "pin" : "pin.slash",
                        label: mode == .floating ? "Dock Panel" : "Float Panel",
                        action: onTogglePin
                    )
                }
                if chrome.shows(.close), let onClose {
                    control(symbol: "xmark", label: "Close", action: onClose)
                }
            }
        }
        .padding(.leading, UIMetrics.spacing4)
        .padding(.trailing, UIMetrics.spacing2)
        .frame(height: UIMetrics.scaled(32))
        .background(MuxyTheme.bg)
    }

    private func customButton(_ button: PanelHeaderButton) -> some View {
        IconButton(
            symbol: button.symbol,
            color: button.isActive ? MuxyTheme.accent : MuxyTheme.fgMuted,
            hoverColor: button.isActive ? MuxyTheme.accent : MuxyTheme.fg,
            accessibilityLabel: button.label,
            action: button.action
        )
        .help(button.label)
    }

    private func control(symbol: String, label: String, action: @escaping () -> Void) -> some View {
        IconButton(symbol: symbol, accessibilityLabel: label, action: action)
            .help(label)
    }

    private var positionSymbol: String {
        switch position {
        case .right: "rectangle.bottomhalf.inset.filled"
        case .bottom: "rectangle.righthalf.inset.filled"
        }
    }

    private var positionLabel: String {
        "Move to \(position.opposite.displayName)"
    }
}
