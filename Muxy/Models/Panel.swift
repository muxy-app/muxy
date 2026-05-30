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
    case close
    case pin
    case position
}

struct PanelHeaderButton: Identifiable, Equatable {
    let id: String
    let symbol: String
    let label: String
    let isActive: Bool
    let action: () -> Void

    init(
        id: String,
        symbol: String,
        label: String,
        isActive: Bool = false,
        action: @escaping () -> Void
    ) {
        self.id = id
        self.symbol = symbol
        self.label = label
        self.isActive = isActive
        self.action = action
    }

    static func == (lhs: PanelHeaderButton, rhs: PanelHeaderButton) -> Bool {
        lhs.id == rhs.id
            && lhs.symbol == rhs.symbol
            && lhs.label == rhs.label
            && lhs.isActive == rhs.isActive
    }
}

struct PanelChrome {
    let iconSymbol: String?
    let title: String?
    let hiddenControls: Set<PanelHeaderControl>
    let trailingButtons: [PanelHeaderButton]

    init(
        iconSymbol: String? = nil,
        title: String? = nil,
        hiddenControls: Set<PanelHeaderControl> = [],
        trailingButtons: [PanelHeaderButton] = []
    ) {
        self.iconSymbol = iconSymbol
        self.title = title
        self.hiddenControls = hiddenControls
        self.trailingButtons = trailingButtons
    }

    func shows(_ control: PanelHeaderControl) -> Bool {
        !hiddenControls.contains(control)
    }

    var hasHeaderContent: Bool {
        iconSymbol != nil
            || title != nil
            || !trailingButtons.isEmpty
            || !hiddenControls.isSuperset(of: PanelHeaderControl.allCases)
    }
}
