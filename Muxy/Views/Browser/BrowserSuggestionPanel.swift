import AppKit
import SwiftUI

@MainActor
final class BrowserSuggestionPanel {
    private let model: BrowserSuggestionModel
    private let panel: NSPanel
    private let hostingView: NSHostingView<BrowserSuggestionList>
    private weak var anchorView: NSView?

    init(model: BrowserSuggestionModel, onSelect: @escaping (BrowserHistoryEntry) -> Void) {
        self.model = model
        hostingView = NSHostingView(rootView: BrowserSuggestionList(model: model, onSelect: onSelect))
        panel = NSPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = true
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.contentView = hostingView
    }

    var isVisible: Bool { panel.isVisible }

    private var horizontalInset: CGFloat = 0
    private var verticalGap: CGFloat = 0

    func show(below anchorView: NSView, horizontalInset: CGFloat, verticalGap: CGFloat) {
        self.anchorView = anchorView
        self.horizontalInset = horizontalInset
        self.verticalGap = verticalGap
        guard let parentWindow = anchorView.window else { return }
        reposition()
        guard panel.parent == nil else { return }
        parentWindow.addChildWindow(panel, ordered: .above)
    }

    func reposition() {
        guard let anchorView, let parentWindow = anchorView.window else { return }
        let width = anchorView.bounds.width + horizontalInset * 2
        let height = max(hostingView.fittingSize.height, 1)

        let anchorRectInWindow = anchorView.convert(anchorView.bounds, to: nil)
        let topLeftInWindow = NSPoint(x: anchorRectInWindow.minX - horizontalInset, y: anchorRectInWindow.minY)
        let topLeftInScreen = parentWindow.convertPoint(toScreen: topLeftInWindow)

        panel.setContentSize(NSSize(width: width, height: height))
        panel.setFrameTopLeftPoint(NSPoint(x: topLeftInScreen.x, y: topLeftInScreen.y - verticalGap))
    }

    func hide() {
        guard panel.parent != nil else { return }
        panel.parent?.removeChildWindow(panel)
        panel.orderOut(nil)
    }
}
