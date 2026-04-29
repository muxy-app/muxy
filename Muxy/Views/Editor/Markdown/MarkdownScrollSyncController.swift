import AppKit

@MainActor
final class MarkdownScrollSyncController {
    private let state: EditorTabState
    private weak var scrollView: NSScrollView?
    private weak var viewport: ViewportState?

    private var isApplyingScroll = false
    private var lastAppliedScrollRequestVersion = 0

    init(state: EditorTabState) {
        self.state = state
    }

    func attach(scrollView: NSScrollView?, viewport: ViewportState?) {
        self.scrollView = scrollView
        self.viewport = viewport
    }

    var isSplitActive: Bool {
        state.isMarkdownFile && state.markdownViewMode == .split && state.markdownScrollSyncEnabled
    }

    func updateEditorScrollMetrics() {
        guard isSplitActive,
              let scrollView,
              let viewport
        else { return }

        let visibleHeight = scrollView.contentView.bounds.height
        let documentHeight = scrollView.documentView?.bounds.height ?? 0
        let maxScrollY = max(0, documentHeight - visibleHeight)
        let scrollY = min(max(0, scrollView.contentView.bounds.origin.y), maxScrollY)

        state.markdownEditorScrollY = scrollY
        state.markdownEditorMaxScrollY = maxScrollY
        state.markdownEditorViewportHeight = visibleHeight
        state.markdownEditorLineHeight = viewport.estimatedLineHeight
    }

    func syncScrollPositionIfNeeded(refreshViewport: () -> Void, rebuildLineStartOffsets: () -> Void) {
        guard state.isMarkdownFile,
              state.markdownViewMode == .split,
              state.markdownScrollSyncEnabled
        else { return }

        applyPendingScrollRequestIfNeeded(
            refreshViewport: refreshViewport,
            rebuildLineStartOffsets: rebuildLineStartOffsets
        )
    }

    func updatePreviewSyncPointFromEditorScroll() {
        guard !isApplyingScroll else { return }
        guard state.markdownScrollDriver != .preview else { return }
        guard state.isMarkdownFile,
              state.markdownViewMode == .split,
              state.markdownScrollSyncEnabled
        else { return }

        let map = state.currentMarkdownSyncMap()
        let output = state.markdownSyncCoordinator.editorDidScroll(scrollY: state.markdownEditorScrollY, map: map)
        guard !output.isEmpty else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.state.applyMarkdownSyncOutput(output)
        }
    }

    func reconcileScrollBoundsChange() {
        updateEditorScrollMetrics()
        if !isSplitActive {
            isApplyingScroll = false
        } else if isApplyingScroll {
            isApplyingScroll = false
        } else {
            if state.markdownScrollDriver != .editor {
                state.markdownScrollDriver = .editor
            }
            updatePreviewSyncPointFromEditorScroll()
        }
    }

    func publishProgressIfEditorAutoScrolled(_ work: () -> Void) {
        guard let scrollView else {
            work()
            return
        }

        let beforeY = scrollView.contentView.bounds.origin.y
        work()
        let afterY = scrollView.contentView.bounds.origin.y

        guard abs(afterY - beforeY) > 0.5 else { return }
        updateEditorScrollMetrics()
        updatePreviewSyncPointFromEditorScroll()
    }

    private func applyPendingScrollRequestIfNeeded(
        refreshViewport: () -> Void,
        rebuildLineStartOffsets: () -> Void
    ) {
        guard let scrollView, viewport != nil else { return }
        guard lastAppliedScrollRequestVersion != state.markdownEditorScrollRequestVersion else { return }
        lastAppliedScrollRequestVersion = state.markdownEditorScrollRequestVersion

        guard state.isMarkdownFile,
              state.markdownViewMode == .split,
              state.markdownScrollSyncEnabled,
              let targetY = state.markdownEditorScrollRequestY
        else { return }

        isApplyingScroll = true
        let visibleHeight = scrollView.contentView.bounds.height
        let documentHeight = scrollView.documentView?.bounds.height ?? 0
        let maxScrollY = max(0, documentHeight - visibleHeight)
        let clamped = min(max(0, targetY), maxScrollY)
        scrollView.contentView.setBoundsOrigin(NSPoint(x: scrollView.contentView.bounds.origin.x, y: clamped))
        scrollView.reflectScrolledClipView(scrollView.contentView)
        refreshViewport()
        rebuildLineStartOffsets()
    }
}
