import AppKit
import Foundation
import UniformTypeIdentifiers

@MainActor
enum RichInputSubmitter {
    private static let imagePasteDelay: Duration = .milliseconds(300)
    private static let initialDelay: Duration = .milliseconds(50)

    enum Segment: Equatable {
        case text(String)
        case image(URL)
    }

    static func submit(richInput: RichInputState, paneIDs: [UUID], appendReturn: Bool) {
        guard !paneIDs.isEmpty else { return }
        let body = richInput.text
        let fileAttachments = richInput.fileAttachments
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

        let segments = resolveSegments(
            text: combined,
            images: imageAttachments,
            strategy: EditorSettings.shared.richInputImageStrategy
        )

        let views = paneIDs.compactMap { TerminalViewRegistry.shared.existingView(for: $0) }
        guard !views.isEmpty else { return }
        let hasImageSegment = segments.contains { if case .image = $0 { true } else { false } }
        let focusTarget = views.count == 1 ? views.first : nil

        if !hasImageSegment {
            let payload = textOnlyPayload(segments: segments, appendReturn: appendReturn)
            Task { @MainActor in
                for view in views {
                    view.clearTerminalInput()
                }
                try? await Task.sleep(for: initialDelay)
                for view in views {
                    view.sendRemoteBytes(payload)
                }
                if let focusTarget {
                    focusTarget.window?.makeFirstResponder(focusTarget)
                }
            }
            return
        }

        Task { @MainActor in
            for view in views {
                view.clearTerminalInput()
            }
            try? await Task.sleep(for: initialDelay)

            let savedClipboard = SystemPasteboardSnapshot.capture()

            for segment in segments {
                switch segment {
                case let .text(chunk):
                    guard !chunk.isEmpty else { continue }
                    for view in views {
                        view.submitRichInput(text: chunk)
                    }
                case let .image(url):
                    for view in views {
                        view.pasteImageURL(url)
                    }
                    try? await Task.sleep(for: imagePasteDelay)
                }
            }

            if appendReturn {
                for view in views {
                    view.sendRemoteBytes(TerminalControlBytes.carriageReturn)
                }
            }

            try? await Task.sleep(for: imagePasteDelay)
            SystemPasteboardSnapshot.restore(items: savedClipboard)

            if let focusTarget {
                focusTarget.window?.makeFirstResponder(focusTarget)
            }
        }
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

    nonisolated static func resolveSegments(
        text: String,
        images: [URL],
        strategy: RichInputImageStrategy
    ) -> [Segment] {
        let raw = tokenize(text: text, images: images)
        guard strategy == .inlinePath else { return raw }
        return raw.map { segment in
            switch segment {
            case .text: segment
            case let .image(url): .text(ShellEscaper.escape(url.path))
            }
        }
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
                if !chunk.isEmpty { segments.append(.text(chunk)) }
            }
            segments.append(.image(images[imageIndex - 1]))
            cursor = match.range.location + match.range.length
        }
        if cursor < length {
            let tail = ns.substring(with: NSRange(location: cursor, length: length - cursor))
            if !tail.isEmpty { segments.append(.text(tail)) }
        }
        return segments
    }
}
