import AppKit

@MainActor
final class QuickTerminalHoverTrackingView: NSView {
    var onEntered: (() -> Void)?
    var onExited: (() -> Void)?

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas {
            removeTrackingArea(area)
        }
        addTrackingArea(
            NSTrackingArea(
                rect: .zero,
                options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                owner: self,
                userInfo: nil
            )
        )
    }

    override func mouseEntered(with _: NSEvent) {
        onEntered?()
    }

    override func mouseExited(with _: NSEvent) {
        onExited?()
    }

    override func mouseDown(with _: NSEvent) {}

    override func rightMouseDown(with _: NSEvent) {}
}

@MainActor
final class QuickTerminalHoverWindow: NSPanel {
    private let trackingView = QuickTerminalHoverTrackingView(frame: .zero)

    var onEntered: (() -> Void)? {
        get { trackingView.onEntered }
        set { trackingView.onEntered = newValue }
    }

    var onExited: (() -> Void)? {
        get { trackingView.onExited }
        set { trackingView.onExited = newValue }
    }

    init() {
        super.init(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        level = .statusBar
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isFloatingPanel = true
        hidesOnDeactivate = false
        isMovable = false
        isMovableByWindowBackground = false
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        ignoresMouseEvents = false
        animationBehavior = .none
        contentView = trackingView
    }

    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}
