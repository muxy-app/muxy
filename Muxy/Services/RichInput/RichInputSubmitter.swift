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

        let pathParts = fileAttachments.map { ShellEscaper.escape($0.path) }
        var combined = ""
        if pathParts.isEmpty {
            combined = body
        } else if trimmedBody.isEmpty {
            combined = pathParts.joined(separator: " ")
        } else {
            combined = pathParts.joined(separator: " ") + " " + body
        }

        let segments = tokenize(text: combined, images: imageAttachments)
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
                    isRemote: imagePasteContext(for: view).isRemote
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
        let hasImages = segments.contains { segment in
            if case .image = segment {
                true
            } else {
                false
            }
        }
        let imageSurface = view as? any TerminalImagePasteSurface
        var imageAttempts: [URL: [TerminalImagePasteAttempt]] = [:]
        if hasImages {
            for segment in segments {
                guard case let .image(url) = segment else { continue }
                guard let attempt = imageSurface?.beginImagePaste() else { return false }
                imageAttempts[url, default: []].append(attempt)
            }
        }
        view.clearTerminalInput(lineBreakCount: 0)
        do {
            try await Task.sleep(for: initialDelay)
        } catch {
            return false
        }
        guard !Task.isCancelled else { return false }

        guard hasImages else {
            view.sendRemoteBytes(textOnlyPayload(segments: segments, appendReturn: appendReturn))
            return true
        }
        guard let imageSurface else { return false }
        var consumedImageCounts: [URL: Int] = [:]
        var submittedLineBreaks = 0
        for segment in segments {
            guard !Task.isCancelled else { return false }
            switch segment {
            case let .text(chunk):
                guard !chunk.isEmpty else { continue }
                view.submitRichInput(text: chunk)
                submittedLineBreaks += chunk.count(where: \.isNewline)
            case let .image(url):
                let attemptIndex = consumedImageCounts[url, default: 0]
                guard let attempts = imageAttempts[url], attemptIndex < attempts.count else {
                    view.clearTerminalInput(lineBreakCount: submittedLineBreaks)
                    return false
                }
                consumedImageCounts[url] = attemptIndex + 1
                let pngData: Data
                do {
                    pngData = try await normalizationBatch.pngData(for: url)
                } catch {
                    guard !Task.isCancelled else { return false }
                    ToastState.shared.show(error.localizedDescription)
                    view.clearTerminalInput(lineBreakCount: submittedLineBreaks)
                    return false
                }
                guard !Task.isCancelled else { return false }
                guard await imageSurface.pasteImageData(pngData, attempt: attempts[attemptIndex]) else {
                    view.clearTerminalInput(lineBreakCount: submittedLineBreaks)
                    return false
                }
            }
        }
        if appendReturn {
            view.sendRemoteBytes(TerminalControlBytes.carriageReturn)
        }
        return true
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

    nonisolated static func segmentsForCapabilities(
        _ segments: [Segment],
        strategy: RichInputImageStrategy,
        capabilities: TerminalCapabilities,
        isRemote: Bool
    ) -> [Segment] {
        resolvedSegments(
            segments,
            strategy: strategy,
            capabilities: capabilities,
            isRemote: isRemote
        )
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

    nonisolated private static func resolvedSegment(
        _ segment: Segment,
        strategy: RichInputImageStrategy,
        capabilities: TerminalCapabilities,
        isRemote: Bool
    ) -> Segment {
        guard case let .image(url) = segment else { return segment }
        if capabilities.contains(.imagePaste), strategy == .clipboard || isRemote {
            return segment
        }
        return .text(ShellEscaper.escape(url.path))
    }

    private static func imagePasteContext(for view: any TerminalSurface) -> WorkspaceContext {
        (view as? any TerminalImagePasteSurface)?.imagePasteWorkspaceContext ?? .local
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
