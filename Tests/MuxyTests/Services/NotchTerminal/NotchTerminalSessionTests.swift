import AppKit
import Testing

@testable import Muxy

@MainActor
@Suite("Notch terminal session")
struct NotchTerminalSessionTests {
    @Test("creates one home-directory surface and retains it while hidden")
    func retainsSurfaceWhileHidden() throws {
        var paths: [String] = []
        var surfaces: [NotchTerminalTestSurface] = []
        let session = NotchTerminalSession(
            homeDirectory: { URL(fileURLWithPath: "/Users/tester") },
            surfaceFactory: { path in
                paths.append(path)
                let surface = NotchTerminalTestSurface()
                surfaces.append(surface)
                return surface
            }
        )

        let first = try #require(session.surfaceForPresentation())
        session.markVisible(false)
        let second = try #require(session.surfaceForPresentation())

        #expect(ObjectIdentifier(first) == ObjectIdentifier(second))
        #expect(paths == ["/Users/tester"])
        #expect(surfaces[0].visibility == [false])
        #expect(surfaces[0].focus == [false])
        #expect(surfaces[0].unfocusedCount == 1)
        #expect(surfaces[0].tearDownCount == 0)
    }

    @Test("process exit tears down and recreates on next presentation")
    func recreatesAfterProcessExit() throws {
        var surfaces: [NotchTerminalTestSurface] = []
        var exitCount = 0
        let session = NotchTerminalSession(surfaceFactory: { _ in
            let surface = NotchTerminalTestSurface()
            surfaces.append(surface)
            return surface
        })
        session.onProcessExit = { exitCount += 1 }

        let first = try #require(session.surfaceForPresentation())
        surfaces[0].onProcessExit?()
        let second = try #require(session.surfaceForPresentation())

        #expect(ObjectIdentifier(first) != ObjectIdentifier(second))
        #expect(surfaces.count == 2)
        #expect(surfaces[0].tearDownCount == 1)
        #expect(exitCount == 1)
    }

    @Test("termination tears down once and blocks recreation")
    func terminationIsIdempotent() throws {
        let surface = NotchTerminalTestSurface()
        let session = NotchTerminalSession(surfaceFactory: { _ in surface })
        _ = try #require(session.surfaceForPresentation())

        session.terminate()
        session.terminate()

        #expect(surface.tearDownCount == 1)
        #expect(session.surfaceForPresentation() == nil)
    }

    @Test("applies appearance to the retained surface")
    func appliesAppearance() throws {
        let surface = NotchTerminalTestSurface()
        let session = NotchTerminalSession(surfaceFactory: { _ in surface })
        let appearance = NotchTerminalAppearance(transparency: 32, blurIntensity: 92)
        _ = try #require(session.surfaceForPresentation())

        session.applyAppearance(appearance)

        #expect(surface.appearances == [appearance])
    }
}

@MainActor
private final class NotchTerminalTestSurface: NotchTerminalSurface {
    let notchTerminalView = NSView()
    var onProcessExit: (() -> Void)?
    var appearances: [NotchTerminalAppearance] = []
    var visibility: [Bool] = []
    var focus: [Bool] = []
    var unfocusedCount = 0
    var tearDownCount = 0

    func applyAppearance(_ appearance: NotchTerminalAppearance) {
        appearances.append(appearance)
    }

    func setVisible(_ visible: Bool) {
        visibility.append(visible)
    }

    func setFocused(_ focused: Bool) {
        focus.append(focused)
    }

    func notifySurfaceUnfocused() {
        unfocusedCount += 1
    }

    func tearDown() {
        tearDownCount += 1
    }
}
