import Foundation

enum OpenerCategory: String, CaseIterable, Identifiable, Codable {
    case projects
    case worktrees
    case layouts
    case branches
    case openTabs

    var id: String { rawValue }

    var label: String {
        switch self {
        case .projects: "Projects"
        case .worktrees: "Worktrees"
        case .layouts: "Layouts"
        case .branches: "Branches"
        case .openTabs: "Open Tabs"
        }
    }

    var symbol: String {
        switch self {
        case .projects: "folder"
        case .worktrees: "point.3.connected.trianglepath.dotted"
        case .layouts: "rectangle.split.2x2"
        case .branches: "arrow.triangle.branch"
        case .openTabs: "macwindow"
        }
    }
}

struct OpenerRecent: Codable, Equatable {
    let key: String
    let category: OpenerCategory
}

enum OpenerPreferences {
    private static let enabledKey = "muxy.opener.enabledCategories"
    private static let recentsKey = "muxy.opener.recents"
    private static let maxRecents = 5

    static var enabledCategories: Set<OpenerCategory> {
        get {
            guard let raw = UserDefaults.standard.array(forKey: enabledKey) as? [String] else {
                return Set(OpenerCategory.allCases)
            }
            let parsed = raw.compactMap { OpenerCategory(rawValue: $0) }
            return parsed.isEmpty ? Set(OpenerCategory.allCases) : Set(parsed)
        }
        set {
            UserDefaults.standard.set(newValue.map(\.rawValue), forKey: enabledKey)
        }
    }

    static var recents: [OpenerRecent] {
        guard let data = UserDefaults.standard.data(forKey: recentsKey) else { return [] }
        return (try? JSONDecoder().decode([OpenerRecent].self, from: data)) ?? []
    }

    static func remember(_ recent: OpenerRecent) {
        var list = recents.filter { $0.key != recent.key }
        list.insert(recent, at: 0)
        if list.count > maxRecents {
            list = Array(list.prefix(maxRecents))
        }
        guard let data = try? JSONEncoder().encode(list) else { return }
        UserDefaults.standard.set(data, forKey: recentsKey)
    }
}
