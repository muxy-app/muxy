import AppKit

@MainActor
final class QuickTerminalHoverController {
    static let defaultDwellInterval: TimeInterval = 0.25

    var onOpenRequested: (() -> Void)?

    private let cutoutScreenProvider: @MainActor () -> NSScreen?
    private let pointerLocationProvider: @MainActor () -> NSPoint
    private let dwellInterval: TimeInterval
    private var window: QuickTerminalHoverWindow?
    private var state = QuickTerminalHoverState()
    private var dwellWorkItem: DispatchWorkItem?
    private(set) var cutoutRect: NSRect?

    init(
        cutoutScreenProvider: @escaping @MainActor () -> NSScreen? = { QuickTerminalCutoutGeometry.firstScreenWithTopInset() },
        pointerLocationProvider: @escaping @MainActor () -> NSPoint = { NSEvent.mouseLocation },
        dwellInterval: TimeInterval = QuickTerminalHoverController.defaultDwellInterval
    ) {
        self.cutoutScreenProvider = cutoutScreenProvider
        self.pointerLocationProvider = pointerLocationProvider
        self.dwellInterval = dwellInterval
    }

    func start() {
        refreshForScreenChange()
    }

    func refreshForScreenChange() {
        guard let screen = cutoutScreenProvider(),
              let rect = QuickTerminalCutoutGeometry.cutoutRect(for: screen)
        else {
            teardownWindow()
            return
        }
        cutoutRect = rect
        let window = ensureWindow()
        window.setFrame(rect, display: false)
        window.orderFrontRegardless()
        state.reset(pointerInside: rect.contains(pointerLocationProvider()))
    }

    func notifyOpened() {
        handle(.terminalOpened)
    }

    func notifyClosed() {
        let inside = cutoutRect?.contains(pointerLocationProvider()) ?? false
        handle(.terminalClosed(pointerInside: inside))
    }

    func tearDown() {
        onOpenRequested = nil
        teardownWindow()
    }

    private func ensureWindow() -> QuickTerminalHoverWindow {
        if let window {
            return window
        }
        let window = QuickTerminalHoverWindow()
        window.onEntered = { [weak self] in self?.handle(.pointerEntered) }
        window.onExited = { [weak self] in self?.handle(.pointerExited) }
        self.window = window
        return window
    }

    private func teardownWindow() {
        cancelDwellTimer()
        cutoutRect = nil
        window?.orderOut(nil)
        window = nil
        state.reset(pointerInside: false)
    }

    private func handle(_ input: QuickTerminalHoverState.Input) {
        apply(state.handle(input))
    }

    private func apply(_ effect: QuickTerminalHoverState.Effect) {
        switch effect {
        case .none:
            break
        case .startDwellTimer:
            startDwellTimer()
        case .cancelDwellTimer:
            cancelDwellTimer()
        case .requestOpen:
            cancelDwellTimer()
            onOpenRequested?()
        }
    }

    private func startDwellTimer() {
        cancelDwellTimer()
        let workItem = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.dwellWorkItem = nil
                self.handle(.dwellElapsed)
            }
        }
        dwellWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + dwellInterval, execute: workItem)
    }

    private func cancelDwellTimer() {
        dwellWorkItem?.cancel()
        dwellWorkItem = nil
    }
}
