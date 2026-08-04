import AppKit
import Testing
@testable import Muxy

@Suite("GhosttyTerminalNSView mouse routing")
@MainActor
struct TerminalMouseRoutingTests {
    @Test func plainLeftPressReachesTheSurface() {
        #expect(GhosttyTerminalNSView.forwardsLeftMousePress(commandHeld: false))
    }

    @Test func commandLeftPressNeverReachesTheSurface() {
        #expect(GhosttyTerminalNSView.forwardsLeftMousePress(commandHeld: true) == false)
    }

    @Test func forwardedPressAlwaysReleases() {
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .forwardedToSurface, didDrag: false))
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .forwardedToSurface, didDrag: true))
    }

    @Test func commandClickReleasesSoLinksStillOpen() {
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .commandPending, didDrag: false))
    }

    @Test func commandDragDoesNotRelease() {
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .commandPending, didDrag: true) == false)
    }

    @Test func handledCommandClickDoesNotRelease() {
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .commandHandled, didDrag: false) == false)
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .commandHandled, didDrag: true) == false)
    }

    @Test func releaseWithoutPressIsDropped() {
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .ignored, didDrag: false) == false)
        #expect(GhosttyTerminalNSView.forwardsLeftMouseRelease(routing: .ignored, didDrag: true) == false)
    }

    @Test func overlayOnlyLetsAForwardedPressRelease() {
        #expect(GhosttyTerminalNSView.reachesSurfaceWhileOverlayActive(routing: .forwardedToSurface))
        #expect(GhosttyTerminalNSView.reachesSurfaceWhileOverlayActive(routing: .commandPending) == false)
        #expect(GhosttyTerminalNSView.reachesSurfaceWhileOverlayActive(routing: .commandHandled) == false)
        #expect(GhosttyTerminalNSView.reachesSurfaceWhileOverlayActive(routing: .ignored) == false)
    }

    @Test func overlayBalancesAForwardedRightPress() {
        #expect(GhosttyTerminalNSView.reachesSurfaceWhileOverlayActive(forwardedPress: true))
        #expect(GhosttyTerminalNSView.reachesSurfaceWhileOverlayActive(forwardedPress: false) == false)
    }

    @Test func rightPressNeverReachesTheSurfaceWithoutMouseReporting() {
        #expect(GhosttyTerminalNSView.forwardsRightMouseButton(mouseCaptured: false, shiftHeld: false) == false)
    }

    @Test func rightPressReachesMouseReportingPrograms() {
        #expect(GhosttyTerminalNSView.forwardsRightMouseButton(mouseCaptured: true, shiftHeld: false))
    }

    @Test func shiftRightClickBypassesMouseReportingPrograms() {
        #expect(GhosttyTerminalNSView.forwardsRightMouseButton(mouseCaptured: true, shiftHeld: true) == false)
    }
}

@Suite("Drag activation")
struct DragActivationTests {
    @Test func jitterWithinTheThresholdIsNotADrag() {
        #expect(DragActivation.reachesDistance(from: CGPoint(x: 100, y: 100), to: CGPoint(x: 102, y: 101)) == false)
    }

    @Test func movementAtExactlyTheThresholdIsADrag() {
        #expect(DragActivation.reachesDistance(
            from: CGPoint(x: 100, y: 100),
            to: CGPoint(x: 100, y: 100 + DragActivation.distance)
        ))
    }

    @Test func movementBeyondTheThresholdIsADrag() {
        #expect(DragActivation.reachesDistance(from: CGPoint(x: 100, y: 100), to: CGPoint(x: 100, y: 106)))
    }
}

@Suite("TabAreaView command drag")
struct TabAreaCommandDragTests {
    @Test func commandHeldAtGestureStartActivatesAtTheThreshold() {
        var activation = CommandDragActivation()

        let activatesAtStart = activation.shouldActivate(
            commandHeld: true,
            from: CGPoint(x: 100, y: 100),
            to: CGPoint(x: 100, y: 100)
        )
        let activatesAtThreshold = activation.shouldActivate(
            commandHeld: true,
            from: CGPoint(x: 100, y: 100),
            to: CGPoint(x: 100, y: 100 + DragActivation.distance)
        )

        #expect(activatesAtStart == false)
        #expect(activatesAtThreshold)
    }

    @Test func commandPressedAfterGestureStartNeverActivatesThePaneDrag() {
        var activation = CommandDragActivation()

        let activatesAtStart = activation.shouldActivate(
            commandHeld: false,
            from: CGPoint(x: 100, y: 100),
            to: CGPoint(x: 100, y: 100)
        )
        let activatesAfterCommandPress = activation.shouldActivate(
            commandHeld: true,
            from: CGPoint(x: 100, y: 100),
            to: CGPoint(x: 100, y: 100 + DragActivation.distance)
        )

        #expect(activatesAtStart == false)
        #expect(activatesAfterCommandPress == false)
    }

    @Test func commandReleasedAfterGestureStartStillActivatesThePaneDrag() {
        var activation = CommandDragActivation()

        let activatesAtStart = activation.shouldActivate(
            commandHeld: true,
            from: CGPoint(x: 100, y: 100),
            to: CGPoint(x: 100, y: 100)
        )
        let activatesAfterCommandRelease = activation.shouldActivate(
            commandHeld: false,
            from: CGPoint(x: 100, y: 100),
            to: CGPoint(x: 100, y: 100 + DragActivation.distance)
        )

        #expect(activatesAtStart == false)
        #expect(activatesAfterCommandRelease)
    }
}
