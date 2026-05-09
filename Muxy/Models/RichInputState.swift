import Foundation

@MainActor
@Observable
final class RichInputState {
    var text: String = ""
    var fileAttachments: [URL] = []
    var imageAttachments: [URL] = []
    var imagePlaceholderCounter: Int = 0
    var focusVersion: Int = 0

    func nextImagePlaceholder(for url: URL) -> String {
        imagePlaceholderCounter += 1
        imageAttachments.append(url)
        return "[Image \(imagePlaceholderCounter)]"
    }

    func apply(_ draft: RichInputDraft) {
        text = draft.text
        fileAttachments = draft.fileAttachments
        imageAttachments = draft.imageAttachments
        imagePlaceholderCounter = draft.imagePlaceholderCounter
    }

    var draft: RichInputDraft {
        RichInputDraft(
            text: text,
            fileAttachments: fileAttachments,
            imageAttachments: imageAttachments,
            imagePlaceholderCounter: imagePlaceholderCounter
        )
    }
}
