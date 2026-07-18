import AppKit
import Testing

@testable import Muxy

@Suite("ResizeHandle")
struct ResizeHandleTests {
    @MainActor
    @Test("cursor region owns the resize pointer surface")
    func cursorRegionOwnsResizePointerSurface() {
        let view = ResizeCursorNSView(axis: .horizontal)
        view.frame = NSRect(x: 0, y: 0, width: 10, height: 100)

        #expect(view.cursor === NSCursor.resizeLeftRight)
        #expect(view.hitTest(NSPoint(x: 5, y: 50)) === view)

        view.axis = .vertical

        #expect(view.cursor === NSCursor.resizeUpDown)
    }

    @Test("panel resize hit areas stay inside their panel edge")
    func panelResizeHitAreasStayInsidePanelEdge() {
        #expect(PanelResizeHandle.Edge.leading.hitAreaBias == .leading)
        #expect(PanelResizeHandle.Edge.trailing.hitAreaBias == .trailing)
        #expect(PanelResizeHandle.Edge.top.hitAreaBias == .leading)
        #expect(PanelResizeHandle.Edge.bottom.hitAreaBias == .trailing)
    }
}
