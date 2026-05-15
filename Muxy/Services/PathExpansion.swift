import Foundation

enum PathExpansion {
    static func expandTilde(_ path: String, homeDirectory: String) -> String {
        let trimmedPath = path.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmedPath == "~" { return homeDirectory }
        if trimmedPath.hasPrefix("~/") {
            return homeDirectory + String(trimmedPath.dropFirst())
        }
        return trimmedPath
    }
}
