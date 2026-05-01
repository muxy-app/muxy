import AppKit
import SwiftUI

struct TooltipInfo: Equatable {
    let id: UUID
    let text: String
    let frame: CGRect
}

@MainActor
@Observable
final class TooltipState {
    static let shared = TooltipState()
    var active: TooltipInfo?
    @ObservationIgnored
    private var timers: [UUID: Timer] = [:]
    @ObservationIgnored
    private var mouseDownMonitor: Any?

    private init() {
        mouseDownMonitor = NSEvent.addLocalMonitorForEvents(matching: [
            .leftMouseDown,
            .rightMouseDown,
            .otherMouseDown,
        ]) { [weak self] event in
            Task { @MainActor in
                self?.hideAll()
            }
            return event
        }
    }

    func show(id: UUID, text: String, frame: CGRect) {
        timers[id]?.invalidate()
        timers[id] = Timer.scheduledTimer(withTimeInterval: 0.2, repeats: false) { [weak self] _ in
            DispatchQueue.main.async {
                self?.active = TooltipInfo(id: id, text: text, frame: frame)
            }
        }
    }

    func hide(id: UUID) {
        timers[id]?.invalidate()
        timers[id] = nil
        if active?.id == id { active = nil }
    }

    func hideAll() {
        timers.values.forEach { $0.invalidate() }
        timers.removeAll()
        active = nil
    }
}

struct QuickTooltipModifier: ViewModifier {
    let text: String
    @State private var id = UUID()
    @State private var globalFrame: CGRect = .zero

    func body(content: Content) -> some View {
        content
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: { globalFrame = $0 }
            .background(TooltipHoverTrackingView { hovering in
                if hovering { TooltipState.shared.show(id: id, text: text, frame: globalFrame) }
                else { TooltipState.shared.hide(id: id) }
            })
            .simultaneousGesture(TapGesture().onEnded {
                TooltipState.shared.hide(id: id)
            })
            .onDisappear {
                TooltipState.shared.hide(id: id)
            }
    }
}

private struct TooltipHoverTrackingView: NSViewRepresentable {
    let onHover: (Bool) -> Void

    func makeNSView(context: Context) -> TooltipTrackingNSView {
        let view = TooltipTrackingNSView()
        view.onHover = onHover
        return view
    }

    func updateNSView(_ nsView: TooltipTrackingNSView, context: Context) {
        nsView.onHover = onHover
    }
}

private final class TooltipTrackingNSView: NSView {
    var onHover: ((Bool) -> Void)?

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas {
            removeTrackingArea(area)
        }
        addTrackingArea(NSTrackingArea(
            rect: bounds,
            options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
            owner: self,
            userInfo: nil
        ))
    }

    override func mouseEntered(with event: NSEvent) {
        onHover?(true)
    }

    override func mouseExited(with event: NSEvent) {
        onHover?(false)
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }
}

struct TooltipOverlay: View {
    @State private var state = TooltipState.shared

    var body: some View {
        GeometryReader { windowGeo in
            if let info = state.active {
                let originX = windowGeo.frame(in: .global).minX
                let originY = windowGeo.frame(in: .global).minY
                Text(info.text)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(MuxyTheme.fg)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 5)
                    .background(
                        RoundedRectangle(cornerRadius: 6)
                            .fill(MuxyTheme.surface)
                            .overlay(RoundedRectangle(cornerRadius: 6).stroke(MuxyTheme.border, lineWidth: 1))
                            .shadow(color: Color.black.opacity(0.2), radius: 8, x: 0, y: 4)
                    )
                    .position(
                        x: info.frame.midX - originX,
                        y: info.frame.maxY - originY + 14
                    )
                    .transition(.opacity.animation(.easeInOut(duration: 0.15)))
            }
        }
    }
}

extension View {
    func quickTooltip(_ text: String) -> some View {
        modifier(QuickTooltipModifier(text: text))
    }
}
