import Foundation

public struct VCSStatusDTO: Codable, Sendable {
    public let branch: String
    public let aheadCount: Int
    public let behindCount: Int
    public let hasUpstream: Bool
    public let stagedFiles: [GitFileDTO]
    public let changedFiles: [GitFileDTO]
    public let defaultBranch: String?
    public let pullRequest: VCSPullRequestDTO?

    public init(
        branch: String,
        aheadCount: Int,
        behindCount: Int,
        hasUpstream: Bool,
        stagedFiles: [GitFileDTO],
        changedFiles: [GitFileDTO],
        defaultBranch: String?,
        pullRequest: VCSPullRequestDTO?
    ) {
        self.branch = branch
        self.aheadCount = aheadCount
        self.behindCount = behindCount
        self.hasUpstream = hasUpstream
        self.stagedFiles = stagedFiles
        self.changedFiles = changedFiles
        self.defaultBranch = defaultBranch
        self.pullRequest = pullRequest
    }
}

public struct GitFileDTO: Identifiable, Codable, Sendable, Hashable {
    public var id: String { path }
    public let path: String
    public let status: GitFileStatusDTO
    public let isUntracked: Bool

    public init(path: String, status: GitFileStatusDTO, isUntracked: Bool = false) {
        self.path = path
        self.status = status
        self.isUntracked = isUntracked
    }
}

public enum GitFileStatusDTO: String, Codable, Sendable {
    case added
    case modified
    case deleted
    case renamed
    case copied
    case untracked
    case unmerged
}

public struct VCSPullRequestDTO: Codable, Sendable, Hashable {
    public let url: String
    public let number: Int
    public let state: String
    public let isDraft: Bool
    public let baseBranch: String

    public init(url: String, number: Int, state: String, isDraft: Bool, baseBranch: String) {
        self.url = url
        self.number = number
        self.state = state
        self.isDraft = isDraft
        self.baseBranch = baseBranch
    }
}

public struct VCSBranchesDTO: Codable, Sendable {
    public let current: String
    public let locals: [String]
    public let defaultBranch: String?

    public init(current: String, locals: [String], defaultBranch: String?) {
        self.current = current
        self.locals = locals
        self.defaultBranch = defaultBranch
    }
}

public struct VCSCreatePRResultDTO: Codable, Sendable {
    public let url: String
    public let number: Int

    public init(url: String, number: Int) {
        self.url = url
        self.number = number
    }
}
