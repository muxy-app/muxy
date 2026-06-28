import Foundation

enum PanelPosition: String, CaseIterable, Identifiable, Codable {
    case right
    case bottom

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .right: "Right"
        case .bottom: "Bottom"
        }
    }

    var opposite: PanelPosition {
        switch self {
        case .right: .bottom
        case .bottom: .right
        }
    }
}

enum PanelMode: String, CaseIterable, Identifiable, Codable {
    case pinned
    case floating

    var id: String { rawValue }
}

enum PanelHeaderControl: String, CaseIterable, Codable {
    case icon
    case title
    case close
    case pin
    case position
}

enum PanelHeaderButtonIcon: Equatable {
    case symbol(String)
    case extensionIcon(ExtensionIcon, MuxyExtension)

    static func == (lhs: PanelHeaderButtonIcon, rhs: PanelHeaderButtonIcon) -> Bool {
        switch (lhs, rhs) {
        case let (.symbol(left), .symbol(right)):
            left == right
        case let (.extensionIcon(leftIcon, leftExtension), .extensionIcon(rightIcon, rightExtension)):
            leftIcon == rightIcon && leftExtension.id == rightExtension.id
        default:
            false
        }
    }
}

struct PanelHeaderButton: Identifiable, Equatable {
    let id: String
    let icon: PanelHeaderButtonIcon
    let label: String
    let isActive: Bool
    let action: () -> Void

    init(
        id: String,
        icon: PanelHeaderButtonIcon,
        label: String,
        isActive: Bool = false,
        action: @escaping () -> Void
    ) {
        self.id = id
        self.icon = icon
        self.label = label
        self.isActive = isActive
        self.action = action
    }

    static func == (lhs: PanelHeaderButton, rhs: PanelHeaderButton) -> Bool {
        lhs.id == rhs.id
            && lhs.icon == rhs.icon
            && lhs.label == rhs.label
            && lhs.isActive == rhs.isActive
    }
}

struct PanelChrome {
    let iconSymbol: String?
    let title: String?
    let hiddenControls: Set<PanelHeaderControl>
    let trailingButtons: [PanelHeaderButton]
    let hidesHeader: Bool

    init(
        iconSymbol: String? = nil,
        title: String? = nil,
        hiddenControls: Set<PanelHeaderControl> = [],
        trailingButtons: [PanelHeaderButton] = [],
        hidesHeader: Bool = false
    ) {
        self.iconSymbol = iconSymbol
        self.title = title
        self.hiddenControls = hiddenControls
        self.trailingButtons = trailingButtons
        self.hidesHeader = hidesHeader
    }

    func shows(_ control: PanelHeaderControl) -> Bool {
        !hiddenControls.contains(control)
    }

    var showsIcon: Bool {
        iconSymbol != nil && shows(.icon)
    }

    var showsTitle: Bool {
        title != nil && shows(.title)
    }

    var hasHeaderContent: Bool {
        guard !hidesHeader else { return false }
        return showsIcon
            || showsTitle
            || !trailingButtons.isEmpty
            || shows(.close)
            || shows(.pin)
            || shows(.position)
    }
}
