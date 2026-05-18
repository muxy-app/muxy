import Foundation

public enum VCSDiffRowKindDTO: String, Codable, Sendable {
    case hunk
    case context
    case addition
    case deletion
    case collapsed
}

public struct VCSDiffRowDTO: Codable, Sendable, Hashable {
    public let kind: VCSDiffRowKindDTO
    public let oldLineNumber: Int?
    public let newLineNumber: Int?
    public let oldText: String?
    public let newText: String?
    public let text: String

    public init(
        kind: VCSDiffRowKindDTO,
        oldLineNumber: Int?,
        newLineNumber: Int?,
        oldText: String?,
        newText: String?,
        text: String
    ) {
        self.kind = kind
        self.oldLineNumber = oldLineNumber
        self.newLineNumber = newLineNumber
        self.oldText = oldText
        self.newText = newText
        self.text = text
    }
}

public struct VCSDiffDTO: Codable, Sendable {
    public let filePath: String
    public let rows: [VCSDiffRowDTO]
    public let additions: Int
    public let deletions: Int
    public let truncated: Bool
    public let isBinary: Bool

    public init(
        filePath: String,
        rows: [VCSDiffRowDTO],
        additions: Int,
        deletions: Int,
        truncated: Bool,
        isBinary: Bool
    ) {
        self.filePath = filePath
        self.rows = rows
        self.additions = additions
        self.deletions = deletions
        self.truncated = truncated
        self.isBinary = isBinary
    }
}
