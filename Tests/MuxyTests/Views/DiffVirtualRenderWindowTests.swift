import Testing

@testable import Muxy

@Suite("DiffVirtualRenderWindow")
struct DiffVirtualRenderWindowTests {
    @Test("window renders one viewport before and after visible content")
    func windowRendersAroundVisibleContent() {
        let window = DiffVirtualRenderWindow(
            editorHeight: 5000,
            viewportHeight: 600,
            visibleBodyY: 1400,
            minimumHeight: 160
        )

        #expect(window.height == 1800)
        #expect(window.offsetY == 800)
    }

    @Test("window clamps to editor bounds near edges")
    func windowClampsToEditorBounds() {
        let topWindow = DiffVirtualRenderWindow(
            editorHeight: 5000,
            viewportHeight: 600,
            visibleBodyY: 200,
            minimumHeight: 160
        )
        let bottomWindow = DiffVirtualRenderWindow(
            editorHeight: 2000,
            viewportHeight: 600,
            visibleBodyY: 1900,
            minimumHeight: 160
        )

        #expect(topWindow.offsetY == 0)
        #expect(bottomWindow.height == 1800)
        #expect(bottomWindow.offsetY == 200)
    }
}
