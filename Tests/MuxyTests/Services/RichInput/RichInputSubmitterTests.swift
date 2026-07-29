import Foundation
import Testing

@testable import Muxy

@Suite("RichInputSubmitter.tokenize")
struct RichInputSubmitterTests {
    @Test("returns single text segment when no images")
    func textOnly() {
        let segments = RichInputSubmitter.tokenize(text: "hello world", images: [])
        #expect(segments == [.text("hello world")])
    }

    @Test("returns empty when text is empty and no images")
    func empty() {
        let segments = RichInputSubmitter.tokenize(text: "", images: [])
        #expect(segments.isEmpty)
    }

    @Test("splits at image placeholder")
    func singleImage() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "before [Image 1] after", images: [url])
        #expect(segments == [.text("before "), .image(url), .text(" after")])
    }

    @Test("preserves order across multiple images")
    func multipleImages() {
        let a = URL(fileURLWithPath: "/tmp/a.png")
        let b = URL(fileURLWithPath: "/tmp/b.png")
        let segments = RichInputSubmitter.tokenize(
            text: "look [Image 1] then [Image 2]",
            images: [a, b]
        )
        #expect(segments == [
            .text("look "),
            .image(a),
            .text(" then "),
            .image(b),
        ])
    }

    @Test("treats out-of-range placeholder as plain text")
    func unknownPlaceholder() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "hi [Image 7]", images: [url])
        #expect(segments == [.text("hi [Image 7]")])
    }

    @Test("placeholder at start with no leading text")
    func leadingPlaceholder() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "[Image 1] tail", images: [url])
        #expect(segments == [.image(url), .text(" tail")])
    }

    @Test("placeholder at end with no trailing text")
    func trailingPlaceholder() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "head [Image 1]", images: [url])
        #expect(segments == [.text("head "), .image(url)])
    }

    @Test("only-image text resolves to single image segment")
    func onlyImage() {
        let url = URL(fileURLWithPath: "/tmp/a.png")
        let segments = RichInputSubmitter.tokenize(text: "[Image 1]", images: [url])
        #expect(segments == [.image(url)])
    }

    @Test("selected non-empty text is used as submission text")
    func selectedTextOverridesFullText() {
        let selected = RichInputSubmitter.selectedSubmissionText("selected part")
        #expect(selected == "selected part")
    }

    @Test("empty selected text falls back to full text")
    func emptySelectedTextFallsBack() {
        let selected = RichInputSubmitter.selectedSubmissionText(" \n\t ")
        #expect(selected == nil)
    }

    @Test("falls back to an escaped image path without image paste capability")
    func imageCapabilityFallback() {
        let url = URL(fileURLWithPath: "/tmp/image with spaces.png")
        let segments = RichInputSubmitter.segmentsForCapabilities(
            [.image(url)],
            strategy: .clipboard,
            capabilities: [],
            isRemote: false
        )

        #expect(segments == [.text("'/tmp/image with spaces.png'")])
    }

    @Test("preserves image segments with image paste capability")
    func imageCapabilitySupport() {
        let url = URL(fileURLWithPath: "/tmp/image.png")
        let segments = RichInputSubmitter.segmentsForCapabilities(
            [.image(url)],
            strategy: .clipboard,
            capabilities: [.imagePaste],
            isRemote: false
        )

        #expect(segments == [.image(url)])
    }

    @Test("inline paths remain local for a local terminal")
    func localInlinePath() {
        let url = URL(fileURLWithPath: "/tmp/image.png")
        let segments = RichInputSubmitter.segmentsForCapabilities(
            [.image(url)],
            strategy: .inlinePath,
            capabilities: [.imagePaste],
            isRemote: false
        )

        #expect(segments == [.text("/tmp/image.png")])
    }

    @Test("remote terminals upload images for every submission strategy")
    func remoteInlinePath() {
        let url = URL(fileURLWithPath: "/tmp/image.png")
        let segments = RichInputSubmitter.segmentsForCapabilities(
            [.image(url)],
            strategy: .inlinePath,
            capabilities: [.imagePaste],
            isRemote: true
        )

        #expect(segments == [.image(url)])
    }

    @Test("failed image upload clears partial input without sending Return")
    @MainActor
    func failedImageUpload() async {
        let target = RichInputSubmissionTestTarget(pasteSucceeds: false)

        let submitted = await RichInputSubmitter.submitSegments(
            [
                .text("explain "),
                .image(URL(fileURLWithPath: "/tmp/image.png")),
                .text(" after"),
            ],
            to: target,
            appendReturn: true,
            normalizer: { _ in Data([1, 2, 3]) }
        )

        #expect(!submitted)
        #expect(target.events == [
            "clear:0",
            "text:explain ",
            "image",
            "clear:0",
        ])
    }

    @Test("failed image upload clears every submitted line of a multi-line prompt")
    @MainActor
    func failedImageUploadClearsMultipleLines() async {
        let target = RichInputSubmissionTestTarget(pasteSucceeds: false)

        let submitted = await RichInputSubmitter.submitSegments(
            [
                .text("first\nsecond\nthird "),
                .image(URL(fileURLWithPath: "/tmp/image.png")),
            ],
            to: target,
            appendReturn: true,
            normalizer: { _ in Data([1, 2, 3]) }
        )

        #expect(!submitted)
        #expect(target.events == [
            "clear:0",
            "text:first\nsecond\nthird ",
            "image",
            "clear:2",
        ])
    }

    @Test("broadcast reuses unique normalization and serializes pane submissions")
    @MainActor
    func serializedBroadcastReusesNormalization() async {
        let probe = RichInputSubmissionProbe()
        let first = RichInputSubmissionTestTarget(
            identifier: "first",
            pasteSucceeds: true,
            probe: probe
        )
        let second = RichInputSubmissionTestTarget(
            identifier: "second",
            pasteSucceeds: true,
            probe: probe
        )
        let imageURL = URL(fileURLWithPath: "/tmp/shared.png")
        let submissions = [
            RichInputSubmitter.TargetSubmission(
                target: first,
                segments: [.image(imageURL), .image(imageURL)]
            ),
            RichInputSubmitter.TargetSubmission(
                target: second,
                segments: [.image(imageURL), .image(imageURL)]
            ),
        ]

        let enqueued = RichInputSubmitter.enqueueSubmissions(
            submissions,
            appendReturn: true,
            normalizer: { url in
                await probe.normalize(url)
            }
        )
        let firstFollower = first.enqueueFollower()
        let secondFollower = second.enqueueFollower()

        #expect(firstFollower)
        #expect(secondFollower)
        await enqueued.waitUntilFinished()
        await first.waitUntilIdle()
        await second.waitUntilIdle()

        #expect(probe.normalizationCalls[imageURL] == 1)
        #expect(probe.maximumConcurrentPastes == 1)
        #expect(probe.pasteOrder == ["first", "first", "second", "second"])
        #expect(first.events.suffix(2) == ["return", "follower"])
        #expect(second.events.suffix(2) == ["return", "follower"])
    }
}

@MainActor
private final class RichInputSubmissionTestTarget:
    TerminalInputTransactionTarget,
    TerminalImagePasteSurface
{
    let imagePasteWorkspaceContext = WorkspaceContext.local
    private let identifier: String
    private let pasteSucceeds: Bool
    private let probe: RichInputSubmissionProbe?
    private let queue = TerminalInputQueue()
    private(set) var events: [String] = []

    init(
        identifier: String = "target",
        pasteSucceeds: Bool,
        probe: RichInputSubmissionProbe? = nil
    ) {
        self.identifier = identifier
        self.pasteSucceeds = pasteSucceeds
        self.probe = probe
    }

    func sendRemoteBytes(_ bytes: Data) {
        events.append(bytes == TerminalControlBytes.carriageReturn ? "return" : "bytes")
    }

    func submitRichInput(text: String) {
        events.append("text:\(text)")
    }

    func clearTerminalInput(lineBreakCount: Int) {
        events.append("clear:\(lineBreakCount)")
    }

    func enqueueInputTransaction(
        _ operation: @escaping @MainActor () async -> Bool
    ) -> TerminalInputTransactionHandle {
        queue.enqueueTransaction(operation)
    }

    func beginImagePaste() -> TerminalImagePasteAttempt? {
        .local(surfaceGeneration: 0)
    }

    func pasteImageData(_ pngData: Data, attempt: TerminalImagePasteAttempt) async -> Bool {
        events.append("image")
        if let probe {
            await probe.recordPaste(identifier: identifier, data: pngData)
        }
        return pasteSucceeds
    }

    func enqueueFollower() -> Bool {
        queue.deferIfPending { [weak self] in
            self?.events.append("follower")
        }
    }

    func waitUntilIdle() async {
        await queue.waitUntilIdle()
    }
}

@MainActor
private final class RichInputSubmissionProbe {
    private(set) var normalizationCalls: [URL: Int] = [:]
    private(set) var pasteOrder: [String] = []
    private(set) var maximumConcurrentPastes = 0
    private var concurrentPastes = 0

    func normalize(_ url: URL) async -> Data {
        normalizationCalls[url, default: 0] += 1
        await Task.yield()
        return Data([1, 2, 3])
    }

    func recordPaste(identifier: String, data: Data) async {
        #expect(data == Data([1, 2, 3]))
        concurrentPastes += 1
        maximumConcurrentPastes = max(maximumConcurrentPastes, concurrentPastes)
        pasteOrder.append(identifier)
        await Task.yield()
        concurrentPastes -= 1
    }
}
