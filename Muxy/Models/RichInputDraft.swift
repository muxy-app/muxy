import Foundation

struct RichInputDraft: Codable, Equatable {
    var text: String
    var fileAttachments: [URL]
    var imageAttachments: [URL]
    var imagePlaceholderCounter: Int

    static let empty = RichInputDraft(
        text: "",
        fileAttachments: [],
        imageAttachments: [],
        imagePlaceholderCounter: 0
    )

    var isEmpty: Bool {
        text.isEmpty && fileAttachments.isEmpty && imageAttachments.isEmpty
    }
}
