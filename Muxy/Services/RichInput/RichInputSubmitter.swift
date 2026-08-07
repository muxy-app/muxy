import Foundation

@MainActor
final class RichInputImageNormalizationBatch {
    typealias Normalizer = @MainActor (URL) async throws -> Data

    private enum Outcome {
        case success(Data)
        case failure(any Error)
    }

    private let urls: [URL]
    private let normalizer: Normalizer
    private var task: Task<[URL: Outcome], Never>?

    init(urls: [URL], normalizer: @escaping Normalizer) {
        var seenURLs = Set<URL>()
        self.urls = urls.filter { seenURLs.insert($0).inserted }
        self.normalizer = normalizer
    }

    func pngData(for url: URL) async throws -> Data {
        let outcomes = await normalizationTask().value
        guard let outcome = outcomes[url] else {
            throw ImagePasteDataError.missingImage
        }
        switch outcome {
        case let .success(data):
            return data
        case let .failure(error):
            throw error
        }
    }

    func cancel() {
        task?.cancel()
    }

    private func normalizationTask() -> Task<[URL: Outcome], Never> {
        if let task {
            return task
        }
        let task = Task { @MainActor [urls, normalizer] in
            var outcomes: [URL: Outcome] = [:]
            for url in urls {
                guard !Task.isCancelled else { break }
                do {
                    let data = try await normalizer(url)
                    outcomes[url] = .success(data)
                } catch {
                    outcomes[url] = .failure(error)
                }
            }
            return outcomes
        }
        self.task = task
        return task
    }
}

@MainActor
enum RichInputSubmitter {
    private static let initialDelay: Duration = .milliseconds(50)

    enum Segment: Equatable {
        case text(String)
        case image(URL)
        case file(URL)
    }

    struct TargetSubmission {
        let target: any TerminalInputTransactionTarget
        let segments: [Segment]
    }

    struct EnqueuedSubmissions {
        let handles: [TerminalInputTransactionHandle]
        let normalizationBatch: RichInputImageNormalizationBatch

        @MainActor
        func waitUntilFinished() async {
            for handle in handles {
                _ = await handle.value()
            }
            normalizationBatch.cancel()
        }
    }

    static func submit(
        richInput: RichInputState,
        paneIDs: [UUID],
        appendReturn: Bool,
        selectedText: String? = nil
    ) {
        guard !paneIDs.isEmpty else { return }
        let selectedBody = selectedSubmissionText(selectedText)
        let body = selectedBody ?? richInput.text
        let fileAttachments = selectedBody == nil ? richInput.fileAttachments : []
        let imageAttachments = richInput.imageAttachments
        let trimmedBody = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBody.isEmpty || !fileAttachments.isEmpty || !imageAttachments.isEmpty else { return }

        let segments = combinedSegments(
            fileAttachments: fileAttachments,
            body: body,
            imageAttachments: imageAttachments
        )
        let strategy = EditorSettings.shared.richInputImageStrategy

        let views = paneIDs.compactMap { TerminalViewRegistry.shared.existingView(for: $0) }
        guard !views.isEmpty else { return }
        let focusTarget = views.count == 1 ? views.first : nil
        let targetSubmissions = views.map { view in
            TargetSubmission(
                target: view,
                segments: segmentsForCapabilities(
                    segments,
                    strategy: strategy,
                    capabilities: view.capabilities,
                    isRemote: isRemoteUploadTarget(view)
                )
            )
        }
        let enqueued = enqueueSubmissions(
            targetSubmissions,
            appendReturn: appendReturn
        )

        Task { @MainActor in
            await enqueued.waitUntilFinished()

            if let focusTarget {
                focusTarget.terminalView.window?.makeFirstResponder(focusTarget.terminalView)
            }
        }
    }

    static func enqueueSubmissions(
        _ submissions: [TargetSubmission],
        appendReturn: Bool,
        normalizer: @escaping RichInputImageNormalizationBatch.Normalizer = {
            try await ImagePasteData.pngData(contentsOf: $0)
        }
    ) -> EnqueuedSubmissions {
        let imageURLs: [URL] = submissions.flatMap { submission -> [URL] in
            submission.segments.compactMap { segment in
                guard case let .image(url) = segment else { return nil }
                return url
            }
        }
        let normalizationBatch = RichInputImageNormalizationBatch(
            urls: imageURLs,
            normalizer: normalizer
        )
        var handles: [TerminalInputTransactionHandle] = []
        var precedingHandle: TerminalInputTransactionHandle?
        for submission in submissions {
            let target = submission.target
            let segments = submission.segments
            let dependencyHandle = precedingHandle
            let handle = target.enqueueInputTransaction { [weak target] in
                if let dependencyHandle {
                    _ = await dependencyHandle.value()
                }
                guard !Task.isCancelled, let target else { return false }
                return await submitSegments(
                    segments,
                    to: target,
                    appendReturn: appendReturn,
                    normalizationBatch: normalizationBatch
                )
            }
            handles.append(handle)
            precedingHandle = handle
        }
        return EnqueuedSubmissions(
            handles: handles,
            normalizationBatch: normalizationBatch
        )
    }

    static func submitSegments(
        _ segments: [Segment],
        to view: any TerminalInputSubmissionTarget,
        appendReturn: Bool,
        normalizer: @escaping RichInputImageNormalizationBatch.Normalizer = {
            try await ImagePasteData.pngData(contentsOf: $0)
        }
    ) async -> Bool {
        let normalizationBatch = RichInputImageNormalizationBatch(
            urls: segments.compactMap { segment in
                guard case let .image(url) = segment else { return nil }
                return url
            },
            normalizer: normalizer
        )
        let submitted = await submitSegments(
            segments,
            to: view,
            appendReturn: appendReturn,
            normalizationBatch: normalizationBatch
        )
        normalizationBatch.cancel()
        return submitted
    }

    private static func submitSegments(
        _ segments: [Segment],
        to view: any TerminalInputSubmissionTarget,
        appendReturn: Bool,
        normalizationBatch: RichInputImageNormalizationBatch
    ) async -> Bool {
        let uploadSurface = view as? any TerminalUploadSurface
        var uploadAttempts: [Int: TerminalUploadAttempt] = [:]
        for (index, segment) in segments.enumerated() {
            if case .text = segment {
                continue
            }
            guard let attempt = uploadSurface?.beginUpload() else { return false }
            uploadAttempts[index] = attempt
        }
        view.clearTerminalInput(lineBreakCount: 0)
        do {
            try await Task.sleep(for: initialDelay)
        } catch {
            return false
        }
        guard !Task.isCancelled else { return false }

        guard !uploadAttempts.isEmpty else {
            view.sendRemoteBytes(textOnlyPayload(segments: segments, appendReturn: appendReturn))
            return true
        }
        guard let uploadSurface else { return false }
        var submittedLineBreaks = 0
        for (index, segment) in segments.enumerated() {
            guard !Task.isCancelled else { return false }
            guard let attempt = uploadAttempts[index] else {
                guard case let .text(chunk) = segment, !chunk.isEmpty else { continue }
                view.submitRichInput(text: chunk)
                submittedLineBreaks += chunk.count(where: \.isNewline)
                continue
            }
            let submitted = await submitAttachment(
                segment,
                attempt: attempt,
                to: view,
                uploadSurface: uploadSurface,
                normalizationBatch: normalizationBatch
            )
            guard submitted else {
                view.clearTerminalInput(lineBreakCount: submittedLineBreaks)
                return false
            }
        }
        if appendReturn {
            view.sendRemoteBytes(TerminalControlBytes.carriageReturn)
        }
        return true
    }

    private static func submitAttachment(
        _ segment: Segment,
        attempt: TerminalUploadAttempt,
        to view: any TerminalInputSubmissionTarget,
        uploadSurface: any TerminalUploadSurface,
        normalizationBatch: RichInputImageNormalizationBatch
    ) async -> Bool {
        switch segment {
        case .text:
            return false
        case let .file(url):
            guard let submissionPath = await uploadSurface.submissionPath(
                forFileAt: url,
                attempt: attempt
            )
            else {
                return false
            }
            view.submitRichInput(text: submissionPath)
            return true
        case let .image(url):
            let pngData: Data
            do {
                pngData = try await normalizationBatch.pngData(for: url)
            } catch {
                guard !Task.isCancelled else { return false }
                ToastState.shared.show(error.localizedDescription)
                return false
            }
            guard !Task.isCancelled else { return false }
            return await uploadSurface.pasteImageData(pngData, attempt: attempt)
        }
    }

    nonisolated static func selectedSubmissionText(_ selectedText: String?) -> String? {
        guard let selectedText else { return nil }
        guard !selectedText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        return selectedText
    }

    private static func textOnlyPayload(segments: [Segment], appendReturn: Bool) -> Data {
        var payload = Data()
        for segment in segments {
            guard case let .text(chunk) = segment, !chunk.isEmpty else { continue }
            let sanitized = chunk.replacingOccurrences(of: "\u{1B}[201~", with: "")
            payload.append(TerminalControlBytes.bracketedPasteStart)
            payload.append(Data(sanitized.utf8))
            payload.append(TerminalControlBytes.bracketedPasteEnd)
        }
        if appendReturn {
            payload.append(TerminalControlBytes.carriageReturn)
        }
        return payload
    }

    nonisolated static func combinedSegments(
        fileAttachments: [URL],
        body: String,
        imageAttachments: [URL]
    ) -> [Segment] {
        guard !fileAttachments.isEmpty else {
            return tokenize(text: body, images: imageAttachments)
        }
        var segments = Array(fileAttachments.flatMap { [Segment.file($0), .text(" ")] }.dropLast())
        guard !body.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return segments }
        segments.append(.text(" "))
        segments.append(contentsOf: tokenize(text: body, images: imageAttachments))
        return segments
    }

    nonisolated static func segmentsForCapabilities(
        _ segments: [Segment],
        strategy: RichInputImageStrategy,
        capabilities: TerminalCapabilities,
        isRemote: Bool
    ) -> [Segment] {
        coalesced(resolvedSegments(
            segments,
            strategy: strategy,
            capabilities: capabilities,
            isRemote: isRemote
        ))
    }

    nonisolated static func resolvedSegments(
        _ segments: [Segment],
        strategy: RichInputImageStrategy,
        capabilities: TerminalCapabilities,
        isRemote: Bool
    ) -> [Segment] {
        segments.map {
            resolvedSegment(
                $0,
                strategy: strategy,
                capabilities: capabilities,
                isRemote: isRemote
            )
        }
    }

    nonisolated static func coalesced(_ segments: [Segment]) -> [Segment] {
        var result: [Segment] = []
        for segment in segments {
            guard case let .text(chunk) = segment,
                  case let .text(previous)? = result.last
            else {
                result.append(segment)
                continue
            }
            result[result.count - 1] = .text(previous + chunk)
        }
        return result
    }

    nonisolated private static func resolvedSegment(
        _ segment: Segment,
        strategy: RichInputImageStrategy,
        capabilities: TerminalCapabilities,
        isRemote: Bool
    ) -> Segment {
        switch segment {
        case .text:
            return segment
        case let .file(url):
            guard capabilities.contains(.upload), isRemote else {
                return .text(ShellEscaper.escape(url.path))
            }
            return segment
        case let .image(url):
            guard capabilities.contains(.upload), strategy == .clipboard || isRemote else {
                return .text(ShellEscaper.escape(url.path))
            }
            return segment
        }
    }

    private static func isRemoteUploadTarget(_ view: any TerminalSurface) -> Bool {
        (view as? any TerminalUploadSurface)?.uploadDestination != nil
    }

    nonisolated static func tokenize(text: String, images: [URL]) -> [Segment] {
        guard !images.isEmpty else {
            return text.isEmpty ? [] : [.text(text)]
        }
        var segments: [Segment] = []
        let ns = text as NSString
        var cursor = 0
        let length = ns.length
        let pattern = "\\[Image (\\d+)\\]"
        guard let regex = try? NSRegularExpression(pattern: pattern) else {
            return text.isEmpty ? [] : [.text(text)]
        }
        let matches = regex.matches(in: text, range: NSRange(location: 0, length: length))
        for match in matches {
            guard match.numberOfRanges == 2 else { continue }
            let indexRange = match.range(at: 1)
            let indexString = ns.substring(with: indexRange)
            guard let imageIndex = Int(indexString),
                  imageIndex >= 1,
                  imageIndex <= images.count
            else { continue }
            if match.range.location > cursor {
                let chunk = ns.substring(with: NSRange(location: cursor, length: match.range.location - cursor))
                if !chunk.isEmpty {
                    segments.append(.text(chunk))
                }
            }
            segments.append(.image(images[imageIndex - 1]))
            cursor = match.range.location + match.range.length
        }
        if cursor < length {
            let tail = ns.substring(with: NSRange(location: cursor, length: length - cursor))
            if !tail.isEmpty {
                segments.append(.text(tail))
            }
        }
        return segments
    }
}
