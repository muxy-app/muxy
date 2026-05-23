import AppKit
import Foundation

@MainActor
enum TerminalSegmentInjector {
    private static let imagePasteDelay: Duration = .milliseconds(300)
    private static let initialDelay: Duration = .milliseconds(50)

    struct Options {
        var clearInput: Bool
        var appendReturn: Bool
        var focusWhenSingleView: Bool

        static let submission = Options(clearInput: true, appendReturn: true, focusWhenSingleView: true)
        static let append = Options(clearInput: false, appendReturn: false, focusWhenSingleView: true)
    }

    static func inject(
        segments: [RichInputSubmitter.Segment],
        into views: [GhosttyTerminalNSView],
        options: Options
    ) {
        guard !views.isEmpty else { return }
        let hasImageSegment = segments.contains { if case .image = $0 { true } else { false } }
        let focusTarget = options.focusWhenSingleView && views.count == 1 ? views.first : nil

        if !hasImageSegment {
            injectTextOnly(segments: segments, views: views, focusTarget: focusTarget, options: options)
            return
        }

        injectWithImages(segments: segments, views: views, focusTarget: focusTarget, options: options)
    }

    private static func injectTextOnly(
        segments: [RichInputSubmitter.Segment],
        views: [GhosttyTerminalNSView],
        focusTarget: GhosttyTerminalNSView?,
        options: Options
    ) {
        let payload = textOnlyPayload(segments: segments, appendReturn: options.appendReturn)
        Task { @MainActor in
            if options.clearInput {
                for view in views {
                    view.clearTerminalInput()
                }
                try? await Task.sleep(for: initialDelay)
            }
            for view in views {
                view.sendRemoteBytes(payload)
            }
            focusTarget?.window?.makeFirstResponder(focusTarget)
        }
    }

    private static func injectWithImages(
        segments: [RichInputSubmitter.Segment],
        views: [GhosttyTerminalNSView],
        focusTarget: GhosttyTerminalNSView?,
        options: Options
    ) {
        Task { @MainActor in
            if options.clearInput {
                for view in views {
                    view.clearTerminalInput()
                }
                try? await Task.sleep(for: initialDelay)
            }

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

            if options.appendReturn {
                for view in views {
                    view.sendRemoteBytes(TerminalControlBytes.carriageReturn)
                }
            }

            try? await Task.sleep(for: imagePasteDelay)
            SystemPasteboardSnapshot.restore(items: savedClipboard)

            focusTarget?.window?.makeFirstResponder(focusTarget)
        }
    }

    private static func textOnlyPayload(segments: [RichInputSubmitter.Segment], appendReturn: Bool) -> Data {
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
}
