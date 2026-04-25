import CoreGraphics
import Foundation

struct MarkdownPreviewAnchorGeometry: Codable, Equatable {
    let anchorID: String
    let startLine: Int?
    let endLine: Int?
    let top: CGFloat
    let height: CGFloat

    var bottom: CGFloat { top + height }
}
