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
        #expect(ResizeHandle.Edge.leading.hitAreaBias == .leading)
        #expect(ResizeHandle.Edge.trailing.hitAreaBias == .trailing)
        #expect(ResizeHandle.Edge.top.hitAreaBias == .leading)
        #expect(ResizeHandle.Edge.bottom.hitAreaBias == .trailing)
    }

    @Test("each resize edge drives its own axis and direction")
    func resizeEdgesDriveTheirAxisAndDirection() {
        #expect(ResizeHandle.Edge.leading.axis == .horizontal)
        #expect(ResizeHandle.Edge.trailing.axis == .horizontal)
        #expect(ResizeHandle.Edge.top.axis == .vertical)
        #expect(ResizeHandle.Edge.bottom.axis == .vertical)

        #expect(ResizeHandle.Edge.leading.isLeading)
        #expect(ResizeHandle.Edge.top.isLeading)
        #expect(!ResizeHandle.Edge.trailing.isLeading)
        #expect(!ResizeHandle.Edge.bottom.isLeading)
    }
}
