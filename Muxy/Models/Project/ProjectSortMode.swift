import Foundation

enum ProjectSortMode: String, CaseIterable, Identifiable {
    case manual
    case nameAscending
    case nameDescending
    case recentlyActive
    case dateCreated

    var id: String { rawValue }

    var title: String {
        switch self {
        case .manual: "Manual"
        case .nameAscending: "Name (A–Z)"
        case .nameDescending: "Name (Z–A)"
        case .recentlyActive: "Recently Active"
        case .dateCreated: "Date Added"
        }
    }

    var systemImage: String {
        switch self {
        case .manual: "hand.draw"
        case .nameAscending: "textformat.abc"
        case .nameDescending: "textformat.abc.dottedunderline"
        case .recentlyActive: "clock.arrow.circlepath"
        case .dateCreated: "calendar"
        }
    }

    static let storageKey = "muxy.projectSortMode"
    static let defaultValue: ProjectSortMode = .manual

    static var current: ProjectSortMode {
        guard let raw = UserDefaults.standard.string(forKey: storageKey),
              let mode = ProjectSortMode(rawValue: raw)
        else { return defaultValue }
        return mode
    }

    func sorted(_ projects: [Project]) -> [Project] {
        let ordered = ordered(projects)
        let pinned = ordered.filter(\.isPinned)
        let rest = ordered.filter { !$0.isPinned }
        return pinned + rest
    }

    private func ordered(_ projects: [Project]) -> [Project] {
        switch self {
        case .manual:
            projects
        case .nameAscending:
            projects.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        case .nameDescending:
            projects.sorted { $0.name.localizedStandardCompare($1.name) == .orderedDescending }
        case .recentlyActive:
            projects.sorted { lhs, rhs in
                switch (lhs.lastActiveAt, rhs.lastActiveAt) {
                case let (l?, r?): l > r
                case (_?, nil): true
                case (nil, _?): false
                case (nil, nil): lhs.sortOrder < rhs.sortOrder
                }
            }
        case .dateCreated:
            projects.sorted { $0.createdAt < $1.createdAt }
        }
    }
}
