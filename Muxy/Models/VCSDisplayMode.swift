import Foundation

enum VCSDisplayMode: String, CaseIterable, Identifiable {
    case tab
    case window
    case attached

    var id: String { rawValue }

    var title: String {
        switch self {
        case .tab: "Tab"
        case .window: "Window"
        case .attached: "Attached"
        }
    }

    private static let key = "muxy.vcsDisplayMode"

    static var current: VCSDisplayMode {
        guard let raw = UserDefaults.standard.string(forKey: key),
              let mode = VCSDisplayMode(rawValue: raw)
        else { return .attached }
        return mode
    }

    func route(tab: () -> Void, window: () -> Void, attached: () -> Void) {
        switch self {
        case .tab:
            tab()
        case .window:
            window()
        case .attached:
            attached()
        }
    }
}
