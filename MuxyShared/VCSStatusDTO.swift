import Foundation

public struct VCSStatusDTO: Codable, Sendable {
    public let branch: String
    public let aheadCount: Int
    public let behindCount: Int
    public let stagedFiles: [GitFileDTO]
    public let changedFiles: [GitFileDTO]

    public init(
        branch: String,
        aheadCount: Int,
        behindCount: Int,
        stagedFiles: [GitFileDTO],
        changedFiles: [GitFileDTO]
    ) {
        self.branch = branch
        self.aheadCount = aheadCount
        self.behindCount = behindCount
        self.stagedFiles = stagedFiles
        self.changedFiles = changedFiles
    }
}

public struct GitFileDTO: Identifiable, Codable, Sendable {
    public var id: String { path }
    public let path: String
    public let status: GitFileStatusDTO

    public init(path: String, status: GitFileStatusDTO) {
        self.path = path
        self.status = status
    }
}

public enum GitFileStatusDTO: String, Codable, Sendable {
    case added
    case modified
    case deleted
    case renamed
    case copied
    case untracked
}
