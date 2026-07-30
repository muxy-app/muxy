import Foundation

public enum FileEncodingDTO: String, Codable, Sendable {
    case utf8
    case base64
}

public struct FileEntryDTO: Codable, Sendable, Equatable {
    public let name: String
    public let path: String
    public let isDirectory: Bool
    public let isIgnored: Bool

    public init(name: String, path: String, isDirectory: Bool, isIgnored: Bool) {
        self.name = name
        self.path = path
        self.isDirectory = isDirectory
        self.isIgnored = isIgnored
    }
}

public struct FileContentDTO: Codable, Sendable, Equatable {
    public let path: String
    public let content: String
    public let size: Int
    public let encoding: FileEncodingDTO

    public init(path: String, content: String, size: Int, encoding: FileEncodingDTO) {
        self.path = path
        self.content = content
        self.size = size
        self.encoding = encoding
    }
}

public struct FileStatDTO: Codable, Sendable, Equatable {
    public let name: String
    public let path: String
    public let isDirectory: Bool
    public let size: Int

    public init(name: String, path: String, isDirectory: Bool, size: Int) {
        self.name = name
        self.path = path
        self.isDirectory = isDirectory
        self.size = size
    }
}

public struct FileChangedEventDTO: Codable, Sendable, Equatable {
    public static let pathLimit = 200

    public let projectID: UUID
    public let worktreeID: UUID?
    public let paths: [String]
    public let truncated: Bool

    public init(projectID: UUID, worktreeID: UUID?, paths: [String], truncated: Bool) {
        self.projectID = projectID
        self.worktreeID = worktreeID
        self.paths = paths
        self.truncated = truncated
    }

    public static func capped(projectID: UUID, worktreeID: UUID?, paths: [String]) -> FileChangedEventDTO {
        FileChangedEventDTO(
            projectID: projectID,
            worktreeID: worktreeID,
            paths: Array(paths.prefix(pathLimit)),
            truncated: paths.count > pathLimit
        )
    }
}
