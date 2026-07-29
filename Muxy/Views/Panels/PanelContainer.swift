import SwiftUI

struct PanelContainer<Content: View>: View {
    let chrome: PanelChrome
    let mode: PanelMode
    let position: PanelPosition
    let focusRestorationID: String?
    let onClose: (() -> Void)?
    let onTogglePin: (() -> Void)?
    let onTogglePosition: (() -> Void)?
    @ViewBuilder let content: () -> Content
    @FocusState private var focusedHeaderControl: HeaderFocus?

    private enum HeaderFocus: Hashable {
        case custom(String)
        case position
        case pin
        case close
    }

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
        .onChange(of: focusedHeaderControl) { _, focusedControl in
            guard let focusRestorationID else { return }
            PanelFocusRestoration.shared.setPanelControlFocused(
                panelID: focusRestorationID,
                focused: focusedControl != nil
            )
        }
    }

    private var header: some View {
        HStack(spacing: UIMetrics.spacing2) {
            if chrome.showsIcon, let symbol = chrome.iconSymbol {
                Image(systemName: symbol)
                    .font(.system(size: UIMetrics.fontFootnote, weight: .semibold))
                    .foregroundStyle(MuxyTheme.fgMuted)
            }
            if chrome.showsTitle, let title = chrome.title {
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
                        focus: .position,
                        action: onTogglePosition
                    )
                }
                if chrome.shows(.pin), let onTogglePin {
                    control(
                        symbol: mode == .floating ? "pin" : "pin.slash",
                        label: mode == .floating ? L10n.string("Dock Panel") : L10n.string("Float Panel"),
                        focus: .pin,
                        action: onTogglePin
                    )
                }
                if chrome.shows(.close), let onClose {
                    control(
                        symbol: "xmark",
                        label: L10n.string("Close"),
                        focus: .close,
                        action: onClose
                    )
                }
            }
        }
        .padding(.leading, UIMetrics.spacing4)
        .padding(.trailing, UIMetrics.spacing2)
        .frame(height: UIMetrics.scaled(32))
        .background(MuxyTheme.bg)
    }

    @ViewBuilder
    private func customButton(_ button: PanelHeaderButton) -> some View {
        switch button.icon {
        case let .symbol(name):
            IconButton(
                symbol: name,
                color: button.isActive ? MuxyTheme.accent : MuxyTheme.fgMuted,
                hoverColor: button.isActive ? MuxyTheme.accent : MuxyTheme.fg,
                accessibilityLabel: button.label,
                action: button.action
            )
            .help(button.label)
            .focused($focusedHeaderControl, equals: .custom(button.id))
        case let .extensionIcon(icon, muxyExtension):
            ExtensionIconButton(
                icon: icon,
                muxyExtension: muxyExtension,
                color: button.isActive ? MuxyTheme.accent : MuxyTheme.fgMuted,
                hoverColor: button.isActive ? MuxyTheme.accent : MuxyTheme.fg,
                accessibilityLabel: button.label,
                action: button.action
            )
            .help(button.label)
            .focused($focusedHeaderControl, equals: .custom(button.id))
        }
    }

    private func control(
        symbol: String,
        label: String,
        focus: HeaderFocus,
        action: @escaping () -> Void
    ) -> some View {
        IconButton(symbol: symbol, accessibilityLabel: label, action: action)
            .help(label)
            .focused($focusedHeaderControl, equals: focus)
    }

    private var positionSymbol: String {
        switch position {
        case .right: "rectangle.bottomhalf.inset.filled"
        case .bottom: "rectangle.righthalf.inset.filled"
        }
    }

    private var positionLabel: String {
        L10n.string("Move to \(L10n.string(key: position.opposite.displayName))")
    }
}
