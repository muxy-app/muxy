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

    static let defaultValue: FileTreeSourcePreference = .projectBase
}
