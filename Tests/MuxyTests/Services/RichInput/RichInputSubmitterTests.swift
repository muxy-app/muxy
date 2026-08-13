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
            capabilities: [.upload],
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
            capabilities: [.upload],
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
            capabilities: [.upload],
            isRemote: true
        )

        #expect(segments == [.image(url)])
    }

    @Test("remote terminals upload file attachments")
    func remoteFileAttachment() {
        let url = URL(fileURLWithPath: "/tmp/report.pdf")
        let segments = RichInputSubmitter.segmentsForCapabilities(
            [.file(url)],
            strategy: .inlinePath,
            capabilities: [.upload],
            isRemote: true
        )

        #expect(segments == [.file(url)])
    }

    @Test("local terminals inline the escaped local path for file attachments")
    func localFileAttachment() {
        let url = URL(fileURLWithPath: "/tmp/my report.pdf")
        let segments = RichInputSubmitter.segmentsForCapabilities(
            [.file(url)],
            strategy: .inlinePath,
            capabilities: [.upload],
            isRemote: false
        )

        #expect(segments == [.text("'/tmp/my report.pdf'")])
    }

    @Test("resolved text segments coalesce into a single bracketed paste")
    func coalescesResolvedText() {
        let first = URL(fileURLWithPath: "/tmp/a.pdf")
        let second = URL(fileURLWithPath: "/tmp/b.pdf")
        let segments = RichInputSubmitter.segmentsForCapabilities(
            [.file(first), .text(" "), .file(second), .text(" look")],
            strategy: .inlinePath,
            capabilities: [.upload],
            isRemote: false
        )

        #expect(segments == [.text("/tmp/a.pdf /tmp/b.pdf look")])
    }

    @Test("file attachments precede the body and keep a single separator")
    func combinesAttachmentsWithBody() {
        let first = URL(fileURLWithPath: "/tmp/a.pdf")
        let second = URL(fileURLWithPath: "/tmp/b.pdf")

        #expect(RichInputSubmitter.combinedSegments(
            fileAttachments: [first, second],
            body: "review these",
            imageAttachments: []
        ) == [.file(first), .text(" "), .file(second), .text(" "), .text("review these")])

        #expect(RichInputSubmitter.combinedSegments(
            fileAttachments: [first],
            body: "   ",
            imageAttachments: []
        ) == [.file(first)])

        #expect(RichInputSubmitter.combinedSegments(
            fileAttachments: [],
            body: "plain",
            imageAttachments: []
        ) == [.text("plain")])
    }

    @Test("failed file upload clears partial input without sending Return")
    @MainActor
    func failedFileUpload() async {
        let target = RichInputSubmissionTestTarget(pasteSucceeds: true, uploadSucceeds: false)

        let submitted = await RichInputSubmitter.submitSegments(
            [.file(URL(fileURLWithPath: "/tmp/report.pdf")), .text(" review")],
            to: target,
            appendReturn: true
        )

        #expect(!submitted)
        #expect(!target.events.contains("return"))
        #expect(target.events.last?.hasPrefix("clear:") == true)
    }

    @Test("uploaded file paths are submitted escaped and in order")
    @MainActor
    func submitsUploadedFilePaths() async {
        let target = RichInputSubmissionTestTarget(pasteSucceeds: true)

        let submitted = await RichInputSubmitter.submitSegments(
            [.file(URL(fileURLWithPath: "/tmp/report.pdf")), .text(" review")],
            to: target,
            appendReturn: true
        )

        #expect(submitted)
        #expect(target.events.contains("upload:report.pdf"))
        #expect(target.events.contains("text:/tmp/muxy-uploads.session/report.pdf"))
        #expect(target.events.contains("text: review"))
        #expect(target.events.last == "return")
    }

    @Test("file route drift to a local terminal submits the escaped local path")
    @MainActor
    func localFileAttemptFallback() async {
        let target = RichInputSubmissionTestTarget(
            pasteSucceeds: true,
            uploadAttempt: .local(surfaceGeneration: 0),
            uploadDestination: nil
        )

        let submitted = await RichInputSubmitter.submitSegments(
            [.file(URL(fileURLWithPath: "/tmp/my report.pdf"))],
            to: target,
            appendReturn: false
        )

        #expect(submitted)
        #expect(target.events == ["clear:0", "text:'/tmp/my report.pdf'"])
    }

    @Test("file route drift from local to remote aborts submission")
    @MainActor
    func localToRemoteFileRouteDrift() async {
        let remote = SSHDestination(host: "example.com")
        let target = RichInputSubmissionTestTarget(
            pasteSucceeds: true,
            uploadAttempt: .local(surfaceGeneration: 0),
            uploadDestination: nil,
            destinationAfterClear: remote
        )

        let submitted = await RichInputSubmitter.submitSegments(
            [.file(URL(fileURLWithPath: "/tmp/my report.pdf"))],
            to: target,
            appendReturn: false
        )

        #expect(!submitted)
        #expect(!target.events.contains { $0.hasPrefix("text:") })
    }

    @Test("multi-file route drift aborts the entire path batch")
    @MainActor
    func multiFileRouteDrift() async throws {
        let first = SSHDestination(host: "first.example.com")
        let second = SSHDestination(host: "second.example.com")
        let attempt = TerminalUploadAttempt.remote(RemoteUploadAttempt(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            uploadID: "ABCDEF01-2345-6789-ABCD-EF0123456789",
            surfaceGeneration: 0,
            destination: first
        ))
        let target = RichInputSubmissionTestTarget(
            pasteSucceeds: true,
            uploadAttempt: attempt,
            uploadDestination: first,
            destinationAfterRemotePath: second
        )

        let paths = await target.submissionPaths(
            forFilesAt: [
                URL(fileURLWithPath: "/tmp/first.pdf"),
                URL(fileURLWithPath: "/tmp/second.pdf"),
            ],
            attempt: attempt
        )

        #expect(paths == nil)
        #expect(target.events == ["upload:first.pdf"])
    }

    @Test("multi-file uploads use distinct remote paths for the same extension")
    @MainActor
    func multiFileUploadIDs() async throws {
        let target = RichInputSubmissionTestTarget(pasteSucceeds: true)
        let attempt = try #require(target.beginUpload())

        let paths = await target.submissionPaths(
            forFilesAt: [
                URL(fileURLWithPath: "/tmp/first.pdf"),
                URL(fileURLWithPath: "/tmp/second.pdf"),
            ],
            attempt: attempt
        )

        #expect(paths?.count == 2)
        #expect(Set(target.remoteUploadIDs).count == 2)
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
        let submitted = await enqueued.waitUntilFinished()
        await first.waitUntilIdle()
        await second.waitUntilIdle()

        #expect(submitted)
        #expect(probe.normalizationCalls[imageURL] == 1)
        #expect(probe.maximumConcurrentPastes == 1)
        #expect(probe.pasteOrder == ["first", "first", "second", "second"])
        #expect(first.events.suffix(2) == ["return", "follower"])
        #expect(second.events.suffix(2) == ["return", "follower"])
    }

    @Test("submission completion reports transaction failure")
    @MainActor
    func submissionCompletionReportsFailure() async {
        let target = RichInputSubmissionTestTarget(pasteSucceeds: true, uploadSucceeds: false)
        let submission = RichInputSubmitter.TargetSubmission(
            target: target,
            segments: [.file(URL(fileURLWithPath: "/tmp/report.pdf"))]
        )

        let enqueued = RichInputSubmitter.enqueueSubmissions([submission], appendReturn: true)
        let submitted = await enqueued.waitUntilFinished()

        #expect(!submitted)
        #expect(!target.events.contains("return"))
    }
}

@MainActor
private final class RichInputSubmissionTestTarget:
    TerminalInputTransactionTarget,
    TerminalUploadSurface
{
    private(set) var uploadDestination: SSHDestination?
    private let identifier: String
    private let pasteSucceeds: Bool
    private let uploadSucceeds: Bool
    private let uploadAttempt: TerminalUploadAttempt
    private let destinationAfterClear: SSHDestination?
    private let destinationAfterRemotePath: SSHDestination?
    private let probe: RichInputSubmissionProbe?
    private let queue = TerminalInputQueue()
    private var continuationCount = 0
    private(set) var events: [String] = []
    private(set) var remoteUploadIDs: [String] = []

    init(
        identifier: String = "target",
        pasteSucceeds: Bool,
        uploadSucceeds: Bool = true,
        uploadAttempt: TerminalUploadAttempt = .remote(RemoteUploadAttempt(
            sessionID: "01234567-89AB-CDEF-0123-456789ABCDEF",
            uploadID: "ABCDEF01-2345-6789-ABCD-EF0123456789",
            surfaceGeneration: 0,
            destination: SSHDestination(host: "example.com")
        )),
        uploadDestination: SSHDestination? = SSHDestination(host: "example.com"),
        destinationAfterClear: SSHDestination? = nil,
        destinationAfterRemotePath: SSHDestination? = nil,
        probe: RichInputSubmissionProbe? = nil
    ) {
        self.identifier = identifier
        self.pasteSucceeds = pasteSucceeds
        self.uploadSucceeds = uploadSucceeds
        self.uploadAttempt = uploadAttempt
        self.uploadDestination = uploadDestination
        self.destinationAfterClear = destinationAfterClear
        self.destinationAfterRemotePath = destinationAfterRemotePath
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
        if let destinationAfterClear {
            uploadDestination = destinationAfterClear
        }
    }

    func enqueueInputTransaction(
        _ operation: @escaping @MainActor () async -> Bool
    ) -> TerminalInputTransactionHandle {
        queue.enqueueTransaction(operation)
    }

    func beginUpload() -> TerminalUploadAttempt? {
        uploadAttempt
    }

    func beginUpload(matching attempt: TerminalUploadAttempt) -> TerminalUploadAttempt? {
        guard uploadAttemptPermitsSideEffects(attempt) else { return nil }
        guard case let .remote(remoteAttempt) = attempt else { return attempt }
        continuationCount += 1
        return .remote(RemoteUploadAttempt(
            sessionID: remoteAttempt.sessionID,
            uploadID: "\(remoteAttempt.uploadID)-\(continuationCount)",
            surfaceGeneration: remoteAttempt.surfaceGeneration,
            destination: remoteAttempt.destination
        ))
    }

    func uploadAttemptPermitsSideEffects(_ attempt: TerminalUploadAttempt) -> Bool {
        attempt.matches(destination: uploadDestination)
    }

    func pasteImageData(_ pngData: Data, attempt: TerminalUploadAttempt) async -> Bool {
        events.append("image")
        if let probe {
            await probe.recordPaste(identifier: identifier, data: pngData)
        }
        return pasteSucceeds
    }

    func remotePath(forFileAt url: URL, attempt: TerminalUploadAttempt) async -> String? {
        events.append("upload:\(url.lastPathComponent)")
        guard uploadSucceeds else { return nil }
        if case let .remote(remoteAttempt) = attempt {
            remoteUploadIDs.append(remoteAttempt.uploadID)
        }
        if let destinationAfterRemotePath {
            uploadDestination = destinationAfterRemotePath
        }
        return "/tmp/muxy-uploads.session/\(url.lastPathComponent)"
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
