import Foundation

enum MarkdownSyncAnchorKind: String, Codable {
    case heading
    case paragraph
    case list
    case blockquote
    case fencedCode
    case table
    case thematicBreak
    case image
    case mermaid
    case htmlBlock
    case other
}

struct MarkdownSyncAnchor: Equatable, Identifiable, Codable {
    let id: String
    let kind: MarkdownSyncAnchorKind
    let startLine: Int
    let endLine: Int
}
