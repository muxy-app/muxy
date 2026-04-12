import Foundation

enum GitError: LocalizedError {
    case notGitRepository
    case noUpstreamBranch
    case commandFailed(String)

    var errorDescription: String? {
        switch self {
        case .notGitRepository:
            "This folder is not a Git repository."
        case .noUpstreamBranch:
            "The current branch has no upstream branch on the remote."
        case let .commandFailed(message):
            message
        }
    }
}

struct GitAheadBehind: Equatable {
    let ahead: Int
    let behind: Int
    let hasUpstream: Bool
}

struct PRInfo: Equatable {
    let url: String
    let number: Int
    let state: PRState
    let isDraft: Bool
    let baseBranch: String
    let mergeable: Bool?
    let checks: PRChecks
}

enum PRState: String {
    case open = "OPEN"
    case closed = "CLOSED"
    case merged = "MERGED"
}

struct PRChecks: Equatable {
    let status: PRChecksStatus
    let passing: Int
    let failing: Int
    let pending: Int
    let total: Int
}

enum PRChecksStatus: Equatable {
    case none
    case pending
    case success
    case failure
}

enum PRCreateError: LocalizedError {
    case ghNotInstalled
    case commandFailed(String)

    var errorDescription: String? {
        switch self {
        case .ghNotInstalled:
            "GitHub CLI (gh) is not installed. Install it with `brew install gh`."
        case let .commandFailed(message):
            message
        }
    }
}

enum PRMergeMethod: String, CaseIterable, Identifiable {
    case squash
    case merge
    case rebase

    var id: String { rawValue }

    var ghFlag: String {
        switch self {
        case .merge: "--merge"
        case .squash: "--squash"
        case .rebase: "--rebase"
        }
    }

    var shortLabel: String {
        switch self {
        case .merge: "Merge"
        case .squash: "Squash"
        case .rebase: "Rebase"
        }
    }

    var label: String {
        switch self {
        case .merge: "Merge Commit"
        case .squash: "Squash and Merge"
        case .rebase: "Rebase and Merge"
        }
    }
}

enum GitValidation {
    private static let hexCharacters = CharacterSet(charactersIn: "0123456789abcdefABCDEF")

    static func validateHash(_ hash: String) throws {
        guard !hash.isEmpty,
              hash.count <= 40,
              hash.unicodeScalars.allSatisfy({ hexCharacters.contains($0) })
        else {
            throw GitError.commandFailed("Invalid commit hash.")
        }
    }

    static func validatePath(repoPath: String, relativePath: String) throws {
        let fullPath = (repoPath as NSString).appendingPathComponent(relativePath)
        let resolvedRepo = (repoPath as NSString).standardizingPath
        let resolvedFull = (fullPath as NSString).standardizingPath
        guard resolvedFull.hasPrefix(resolvedRepo + "/") else {
            throw GitError.commandFailed("File path is outside the repository.")
        }
    }
}
