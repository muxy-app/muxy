import AppKit
import SwiftUI

enum FloatingPanelOutsideClickDecision {
    static func shouldDismiss(panelBounds: CGRect, clickLocation: CGPoint) -> Bool {
        !panelBounds.contains(clickLocation)
    }
}

enum PanelLayoutMetrics {
    static let richInputWidthRange: ClosedRange<CGFloat> = 280 ... 800
    static let richInputDefaultWidth: Double = 380
    static let richInputHeightRange: ClosedRange<CGFloat> = 120 ... 600
    static let richInputDefaultHeight: Double = 220

    static let consoleHeightRange: ClosedRange<CGFloat> = 120 ... 600
    static let consoleDefaultHeight: Double = 220

    static let extensionWidthRange: ClosedRange<CGFloat> = 240 ... 800
    static let extensionDefaultWidth: Double = 360
    static let extensionHeightRange: ClosedRange<CGFloat> = 160 ... 600
    static let extensionDefaultHeight: Double = 240
}

struct PanelHostSlot<Content: View>: View {
    let panelHost: PanelHost
    let position: PanelPosition
    let mode: PanelMode
    @ViewBuilder let content: (PanelPlacement) -> Content

    var body: some View {
        if let placement = panelHost.panel(at: position, mode: mode) {
            content(placement)
        }
    }
}

struct PanelFrame: ViewModifier {
    let position: PanelPosition
    let size: Binding<Double>
    let range: ClosedRange<CGFloat>

    func body(content: Content) -> some View {
        switch position {
        case .right:
            HStack(spacing: 0) {
                handle(axis: .horizontal, edge: .leading)
                content.frame(width: CGFloat(size.wrappedValue))
            }
        case .bottom:
            VStack(spacing: 0) {
                handle(axis: .vertical, edge: .top)
                content.frame(height: CGFloat(size.wrappedValue))
            }
        }
    }

    private func handle(axis: ResizeHandle.Axis, edge: ResizeHandle.Edge) -> some View {
        PanelResizeHandle(
            axis: axis,
            edge: edge,
            current: { CGFloat(size.wrappedValue) },
            apply: { next in
                size.wrappedValue = Double(min(range.upperBound, max(range.lowerBound, next)))
            }
        )
    }
}

struct FloatingPanelOutsideClickMonitor: NSViewRepresentable {
    let onOutsideClick: () -> Void

    func makeNSView(context: Context) -> FloatingPanelOutsideClickMonitoringView {
        let view = FloatingPanelOutsideClickMonitoringView()
        view.onOutsideClick = onOutsideClick
        return view
    }

    func updateNSView(_ nsView: FloatingPanelOutsideClickMonitoringView, context: Context) {
        nsView.onOutsideClick = onOutsideClick
    }

    static func dismantleNSView(_ nsView: FloatingPanelOutsideClickMonitoringView, coordinator: ()) {
        nsView.stopMonitoring()
    }
}

final class FloatingPanelOutsideClickMonitoringView: NSView {
    var onOutsideClick: (() -> Void)?
    private var mouseMonitor: Any?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            stopMonitoring()
        } else {
            startMonitoring()
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    func stopMonitoring() {
        guard let mouseMonitor else { return }
        NSEvent.removeMonitor(mouseMonitor)
        self.mouseMonitor = nil
    }

    private func startMonitoring() {
        guard mouseMonitor == nil else { return }
        mouseMonitor = NSEvent.addLocalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]
        ) { [weak self] event in
            self?.handle(event)
            return event
        }
    }

    private func handle(_ event: NSEvent) {
        guard let window, event.window === window else { return }
        let clickLocation = convert(event.locationInWindow, from: nil)
        guard FloatingPanelOutsideClickDecision.shouldDismiss(
            panelBounds: bounds,
            clickLocation: clickLocation
        )
        else { return }
        onOutsideClick?()
    }
}
