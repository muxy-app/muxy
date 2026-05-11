import Foundation

enum VCSKind: String, Codable, Hashable, CaseIterable {
    case git
    case jjColocated
    case jjNative

    static func detect(at path: String) async -> VCSKind? {
        let fm = FileManager.default
        var current = (path as NSString).standardizingPath
        var isDirectory = ObjCBool(false)

        while true {
            let jjPath = (current as NSString).appendingPathComponent(".jj")
            let gitPath = (current as NSString).appendingPathComponent(".git")

            let hasJJ = fm.fileExists(atPath: jjPath, isDirectory: &isDirectory) && isDirectory.boolValue
            let hasGit = fm.fileExists(atPath: gitPath)

            if hasJJ {
                return hasGit ? .jjColocated : .jjNative
            }

            if hasGit {
                return .git
            }

            let parent = (current as NSString).deletingLastPathComponent
            if parent == current {
                break
            }
            current = parent
        }

        return nil
    }

    var isJujutsu: Bool {
        switch self {
        case .jjColocated,
             .jjNative:
            true
        case .git:
            false
        }
    }

    var displayName: String {
        switch self {
        case .git:
            "Git"
        case .jjColocated:
            "Jujutsu (colocated)"
        case .jjNative:
            "Jujutsu"
        }
    }
}
