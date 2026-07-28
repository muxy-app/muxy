import Foundation

@MainActor
enum RichInputSubmitter {
    private static let initialDelay: Duration = .milliseconds(50)

    enum Segment: Equatable {
        case text(String)
        case image(URL)
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

        Task { @MainActor in
            let submissions = views.map { view in
                Task { @MainActor in
                    let resolved = segmentsForCapabilities(
                        segments,
                        strategy: strategy,
                        capabilities: view.capabilities,
                        isRemote: imagePasteContext(for: view).isRemote
                    )
                    return await view.performInputTransaction {
                        await submit(
                            segments: resolved,
                            to: view,
                            appendReturn: appendReturn
                        )
                    }
                }
            }
            for submission in submissions {
                _ = await submission.value
            }

            if let focusTarget {
                focusTarget.terminalView.window?.makeFirstResponder(focusTarget.terminalView)
            }
        }
    }

    private static func submit(
        segments: [Segment],
        to view: any TerminalSurface,
        appendReturn: Bool
    ) async -> Bool {
        view.clearTerminalInput()
        do {
            try await Task.sleep(for: initialDelay)
        } catch {
            return false
        }
        guard !Task.isCancelled else { return false }

        let hasImage = segments.contains {
            if case .image = $0 {
                true
            } else {
                false
            }
        }
        guard hasImage else {
            view.sendRemoteBytes(textOnlyPayload(segments: segments, appendReturn: appendReturn))
            return true
        }
        guard let imageSurface = view as? any TerminalImagePasteSurface else {
            view.clearTerminalInput()
            return false
        }
        for segment in segments {
            guard !Task.isCancelled else { return false }
            switch segment {
            case let .text(chunk):
                guard !chunk.isEmpty else { continue }
                view.submitRichInput(text: chunk)
            case let .image(url):
                guard await imageSurface.pasteImageURL(url) else {
                    view.clearTerminalInput()
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
