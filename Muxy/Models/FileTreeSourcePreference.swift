import Foundation

enum FileTreeSourcePreference: String, CaseIterable, Identifiable {
    case projectBase
    case activeTerminal

    var id: String { rawValue }

    var title: String {
        switch self {
        case .projectBase: "Project base"
        case .activeTerminal: "Active terminal directory"
        }
    }

    static let storageKey = "muxy.fileTreeSource"
    static let defaultValue: FileTreeSourcePreference = .projectBase

    static var current: FileTreeSourcePreference {
        guard let raw = UserDefaults.standard.string(forKey: storageKey),
              let value = FileTreeSourcePreference(rawValue: raw)
        else { return defaultValue }
        return value
    }
}
