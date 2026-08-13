import Foundation

@MainActor
@Observable
final class RichInputState {
    var text: String = "" {
        didSet { advanceDraftRevision(if: text != oldValue) }
    }

    var fileAttachments: [URL] = [] {
        didSet { advanceDraftRevision(if: fileAttachments != oldValue) }
    }

    var imageAttachments: [URL] = [] {
        didSet { advanceDraftRevision(if: imageAttachments != oldValue) }
    }

    var imagePlaceholderCounter: Int = 0 {
        didSet { advanceDraftRevision(if: imagePlaceholderCounter != oldValue) }
    }

    var focusVersion: Int = 0
    @ObservationIgnored private(set) var draftRevision: UInt64 = 0

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

    func clear() {
        text = ""
        fileAttachments = []
        imageAttachments = []
        imagePlaceholderCounter = 0
    }

    @discardableResult
    func clear(ifUnchangedSince submittedRevision: UInt64) -> Bool {
        guard draftRevision == submittedRevision else { return false }
        clear()
        return true
    }

    var draft: RichInputDraft {
        RichInputDraft(
            text: text,
            fileAttachments: fileAttachments,
            imageAttachments: imageAttachments,
            imagePlaceholderCounter: imagePlaceholderCounter
        )
    }

    private func advanceDraftRevision(if draftChanged: Bool) {
        guard draftChanged else { return }
        draftRevision &+= 1
    }
}
