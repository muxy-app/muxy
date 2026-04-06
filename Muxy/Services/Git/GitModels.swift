import Foundation

struct NumstatEntry {
    let additions: Int?
    let deletions: Int?
    let isBinary: Bool
}

struct GitStatusFile: Identifiable, Hashable {
    let path: String
    let oldPath: String?
    let xStatus: Character
    let yStatus: Character
    let additions: Int?
    let deletions: Int?
    let isBinary: Bool

    var id: String { path }

    var isStaged: Bool {
        let staged: Set<Character> = ["A", "M", "D", "R", "C"]
        return staged.contains(xStatus)
    }

    var isUnstaged: Bool {
        let unstaged: Set<Character> = ["M", "D", "?"]
        return unstaged.contains(yStatus) || (xStatus == "?" && yStatus == "?")
    }

    var statusText: String {
        switch (xStatus, yStatus) {
        case ("A", _),
             (_, "A"):
            "A"
        case ("D", _),
             (_, "D"):
            "D"
        case ("R", _),
             (_, "R"):
            "R"
        case ("C", _),
             (_, "C"):
            "C"
        case ("M", _),
             (_, "M"):
            "M"
        case ("U", _),
             (_, "U"):
            "U"
        default:
            "?"
        }
    }

    var stagedStatusText: String {
        String(xStatus)
    }

    var unstagedStatusText: String {
        if xStatus == "?", yStatus == "?" {
            return "U"
        }
        return String(yStatus)
    }
}

struct DiffDisplayRow: Identifiable {
    enum Kind {
        case hunk
        case context
        case addition
        case deletion
        case collapsed
    }

    let id = UUID()
    let kind: Kind
    let oldLineNumber: Int?
    let newLineNumber: Int?
    let oldText: String?
    let newText: String?
    let text: String
}
