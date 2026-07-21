import AppKit

@MainActor
final class NotchHoverController {
    static let defaultDwellInterval: TimeInterval = 0.25

    var onOpenRequested: (() -> Void)?

    private let notchedScreenProvider: @MainActor () -> NSScreen?
    private let pointerLocationProvider: @MainActor () -> NSPoint
    private let dwellInterval: TimeInterval
    private var window: NotchHoverWindow?
    private var state = NotchHoverState()
    private var dwellWorkItem: DispatchWorkItem?
    private(set) var notchRect: NSRect?

    init(
        notchedScreenProvider: @escaping @MainActor () -> NSScreen? = { NotchTerminalNotchGeometry.firstNotchedScreen() },
        pointerLocationProvider: @escaping @MainActor () -> NSPoint = { NSEvent.mouseLocation },
        dwellInterval: TimeInterval = NotchHoverController.defaultDwellInterval
    ) {
        self.notchedScreenProvider = notchedScreenProvider
        self.pointerLocationProvider = pointerLocationProvider
        self.dwellInterval = dwellInterval
    }

    func start() {
        refreshForScreenChange()
    }

    func refreshForScreenChange() {
        guard let screen = notchedScreenProvider(),
              let rect = NotchTerminalNotchGeometry.notchRect(for: screen)
        else {
            teardownWindow()
            return
        }
        notchRect = rect
        let window = ensureWindow()
        window.setFrame(rect, display: false)
        window.orderFrontRegardless()
        state.reset(pointerInside: rect.contains(pointerLocationProvider()))
    }

    func notifyOpened() {
        handle(.terminalOpened)
    }

    func notifyClosed() {
        let inside = notchRect?.contains(pointerLocationProvider()) ?? false
        handle(.terminalClosed(pointerInside: inside))
    }

    func tearDown() {
        onOpenRequested = nil
        teardownWindow()
    }

    private func ensureWindow() -> NotchHoverWindow {
        if let window {
            return window
        }
        let window = NotchHoverWindow()
        window.onEntered = { [weak self] in self?.handle(.pointerEntered) }
        window.onExited = { [weak self] in self?.handle(.pointerExited) }
        self.window = window
        return window
    }

    private func teardownWindow() {
        cancelDwellTimer()
        notchRect = nil
        window?.orderOut(nil)
        window = nil
        state.reset(pointerInside: false)
    }

    private func handle(_ input: NotchHoverState.Input) {
        apply(state.handle(input))
    }

    private func apply(_ effect: NotchHoverState.Effect) {
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
