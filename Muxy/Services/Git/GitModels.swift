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
    let stagedAdditions: Int?
    let stagedDeletions: Int?
    let unstagedAdditions: Int?
    let unstagedDeletions: Int?
    let isBinary: Bool

    init(
        path: String,
        oldPath: String?,
        xStatus: Character,
        yStatus: Character,
        additions: Int?,
        deletions: Int?,
        stagedAdditions: Int? = nil,
        stagedDeletions: Int? = nil,
        unstagedAdditions: Int? = nil,
        unstagedDeletions: Int? = nil,
        isBinary: Bool
    ) {
        self.path = path
        self.oldPath = oldPath
        self.xStatus = xStatus
        self.yStatus = yStatus
        self.additions = additions
        self.deletions = deletions
        self.stagedAdditions = stagedAdditions
        self.stagedDeletions = stagedDeletions
        self.unstagedAdditions = unstagedAdditions
        self.unstagedDeletions = unstagedDeletions
        self.isBinary = isBinary
    }

    var id: String { path }

    var isStaged: Bool {
        let staged: Set<Character> = ["A", "M", "D", "R", "C"]
        return staged.contains(xStatus)
    }

    var isUnstaged: Bool {
        let unstaged: Set<Character> = ["M", "D", "?"]
        return unstaged.contains(yStatus) || (xStatus == "?" && yStatus == "?")
    }

    func additions(isStaged: Bool) -> Int? {
        if isStaged {
            return stagedAdditions ?? additions
        }
        return unstagedAdditions ?? additions
    }

    func deletions(isStaged: Bool) -> Int? {
        if isStaged {
            return stagedDeletions ?? deletions
        }
        return unstagedDeletions ?? deletions
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

    func displayStatusText(isStaged: Bool) -> String {
        let status = isStaged ? stagedStatusText : unstagedStatusText
        if status.trimmingCharacters(in: .whitespaces).isEmpty {
            return statusText
        }
        return status
    }
}

struct GitCommit: Identifiable {
    let hash: String
    let shortHash: String
    let subject: String
    let authorName: String
    let authorDate: Date
    let refs: [GitRef]
    let parentHashes: [String]

    var id: String { hash }
    var isMerge: Bool { parentHashes.count > 1 }
}

struct GitRef {
    enum Kind {
        case localBranch
        case remoteBranch
        case tag
        case head
    }

    let name: String
    let kind: Kind
}

struct DiffDisplayRow: Identifiable {
    enum Kind {
        case hunk
        case context
        case addition
        case deletion
        case collapsed
        case commentSpacer
    }

    let id = UUID()
    let kind: Kind
    let oldLineNumber: Int?
    let newLineNumber: Int?
    let oldText: String?
    let newText: String?
    let text: String
}
