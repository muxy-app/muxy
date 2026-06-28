import Foundation
import Testing

@testable import Muxy

@Suite("BrowserZoom")
struct BrowserZoomTests {
    @Test("zoom in increases by step")
    func zoomInStep() {
        #expect(BrowserZoom.zoomIn(1) == 1.1)
    }

    @Test("zoom in clamps at maximum")
    func zoomInClamps() {
        #expect(BrowserZoom.zoomIn(3) == BrowserZoom.maximum)
        #expect(BrowserZoom.zoomIn(2.9) == BrowserZoom.maximum)
    }

    @Test("zoom out decreases by step")
    func zoomOutStep() {
        #expect(BrowserZoom.zoomOut(1.1) == 1)
    }

    @Test("zoom out clamps at minimum")
    func zoomOutClamps() {
        #expect(BrowserZoom.zoomOut(0.5) == BrowserZoom.minimum)
        #expect(BrowserZoom.zoomOut(0.51) == BrowserZoom.minimum)
    }
}
