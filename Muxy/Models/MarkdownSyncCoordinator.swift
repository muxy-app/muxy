import CoreGraphics
import Foundation

@MainActor
final class MarkdownSyncCoordinator {
    enum Driver {
        case editor
        case preview
    }

    struct Output: Equatable {
        var requestPreviewScrollTop: CGFloat?
        var requestEditorScrollY: CGFloat?

        var isEmpty: Bool {
            requestPreviewScrollTop == nil && requestEditorScrollY == nil
        }
    }

    private let now: () -> TimeInterval

    private var driver: Driver?
    private var driverSince: TimeInterval = 0

    private var lastIssuedPreviewScrollTop: CGFloat?
    private var lastIssuedEditorScrollY: CGFloat?
    private var lastEditorInputScrollY: CGFloat?
    private var lastPreviewInputScrollTop: CGFloat?

    init(now: @escaping () -> TimeInterval = { CFAbsoluteTimeGetCurrent() }) {
        self.now = now
    }

    func editorDidScroll(scrollY: CGFloat, map: MarkdownSyncMap) -> Output {
        guard !map.isEmpty else { return Output() }

        lastEditorInputScrollY = scrollY

        let timestamp = now()
        guard shouldAcceptUpdate(from: .editor, timestamp: timestamp, incoming: scrollY) else {
            return Output()
        }

        let target = map.previewScrollTop(forEditorScrollY: scrollY)
        if let lastIssuedPreviewScrollTop, abs(target - lastIssuedPreviewScrollTop) < 0.5 {
            return Output()
        }

        driver = .editor
        driverSince = timestamp
        lastIssuedPreviewScrollTop = target
        return Output(requestPreviewScrollTop: target)
    }

    func previewDidScroll(scrollTop: CGFloat, map: MarkdownSyncMap) -> Output {
        guard !map.isEmpty else { return Output() }

        lastPreviewInputScrollTop = scrollTop

        let timestamp = now()
        guard shouldAcceptUpdate(from: .preview, timestamp: timestamp, incoming: scrollTop) else {
            return Output()
        }

        let target = map.editorScrollY(forPreviewScrollTop: scrollTop)
        if let lastIssuedEditorScrollY, abs(target - lastIssuedEditorScrollY) < 0.5 {
            return Output()
        }

        driver = .preview
        driverSince = timestamp
        lastIssuedEditorScrollY = target
        return Output(requestEditorScrollY: target)
    }

    func reissueAfterRelayout(map: MarkdownSyncMap) -> Output {
        guard !map.isEmpty else { return Output() }
        guard let driver else { return Output() }

        switch driver {
        case .editor:
            guard let lastEditorInputScrollY else { return Output() }
            let target = map.previewScrollTop(forEditorScrollY: lastEditorInputScrollY)
            lastIssuedPreviewScrollTop = target
            return Output(requestPreviewScrollTop: target)
        case .preview:
            guard let lastPreviewInputScrollTop else { return Output() }
            let target = map.editorScrollY(forPreviewScrollTop: lastPreviewInputScrollTop)
            lastIssuedEditorScrollY = target
            return Output(requestEditorScrollY: target)
        }
    }

    private func shouldAcceptUpdate(from incoming: Driver, timestamp: TimeInterval, incoming value: CGFloat) -> Bool {
        guard let driver else { return true }
        if driver == incoming { return true }

        let suppressionWindow: TimeInterval = 0.18
        guard timestamp - driverSince < suppressionWindow else { return true }

        switch incoming {
        case .editor:
            guard let lastIssuedEditorScrollY else { return true }
            return abs(value - lastIssuedEditorScrollY) > 1.5
        case .preview:
            guard let lastIssuedPreviewScrollTop else { return true }
            return abs(value - lastIssuedPreviewScrollTop) > 1.5
        }
    }
}
