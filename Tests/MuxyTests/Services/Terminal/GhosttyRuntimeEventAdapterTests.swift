import Foundation
import Testing
@testable import Muxy

@Suite("Ghostty runtime event adapter")
@MainActor
struct GhosttyRuntimeEventAdapterTests {
    @Test("delivers an exit callback for the current surface generation")
    func deliversCurrentSurfaceExit() async {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")
        var exitCount = 0
        view.onProcessExit = { exitCount += 1 }

        GhosttyRuntimeEventAdapter().closeSurface(
            userdata: Unmanaged.passUnretained(view).toOpaque(),
            needsConfirm: false
        )
        await drainMainQueue()

        #expect(exitCount == 1)
        #expect(view.processExitHandled)
    }

    @Test("ignores an exit callback from a replaced surface generation")
    func ignoresReplacedSurfaceExit() async {
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")
        var exitCount = 0
        view.onProcessExit = { exitCount += 1 }

        GhosttyRuntimeEventAdapter().closeSurface(
            userdata: Unmanaged.passUnretained(view).toOpaque(),
            needsConfirm: false
        )
        view.destroySurface()
        await drainMainQueue()

        #expect(exitCount == 0)
        #expect(!view.processExitHandled)
    }

    private func drainMainQueue() async {
        await withCheckedContinuation { continuation in
            DispatchQueue.main.async {
                continuation.resume()
            }
        }
    }
}
