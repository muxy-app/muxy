import AppKit
import Testing

@testable import Muxy

@Suite("ReparentingNSViewBroker")
@MainActor
struct ReparentingNSViewHostTests {
    final class Recorder {
        var prepared: [String] = []
        var mounted: [(String, Bool)] = []
        var unmounted: [ObjectIdentifier] = []
    }

    @Test("stale source updates cannot reclaim a destination-owned view")
    func staleSourceUpdateDoesNotReclaimDestinationView() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let sourceClaimID = UUID()
        let destinationClaimID = UUID()
        let sourceHost = ReparentingNSViewHost()
        let destinationHost = ReparentingNSViewHost()
        let view = NSView()

        broker.register(
            claimID: sourceClaimID,
            view: view,
            host: sourceHost,
            configuration: configuration("source", recorder: recorder)
        )
        broker.register(
            claimID: destinationClaimID,
            view: view,
            host: destinationHost,
            configuration: configuration("destination", recorder: recorder)
        )
        recorder.prepared.removeAll()
        recorder.mounted.removeAll()

        broker.update(
            claimID: sourceClaimID,
            view: view,
            host: sourceHost,
            configuration: configuration("stale-source", recorder: recorder)
        )
        broker.release(claimID: sourceClaimID, host: sourceHost)

        #expect(view.superview === destinationHost)
        #expect(sourceHost.subviews.isEmpty)
        #expect(destinationHost.subviews == [view])
        #expect(!recorder.prepared.contains("stale-source"))
        #expect(recorder.prepared.allSatisfy { $0 == "destination" })
        #expect(recorder.unmounted.isEmpty)
    }

    @Test("releasing the active claim restores the newest surviving claim")
    func activeReleaseRestoresSurvivingClaim() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let sourceClaimID = UUID()
        let destinationClaimID = UUID()
        let sourceHost = ReparentingNSViewHost()
        let destinationHost = ReparentingNSViewHost()
        let view = NSView()

        broker.register(
            claimID: sourceClaimID,
            view: view,
            host: sourceHost,
            configuration: configuration("source", recorder: recorder)
        )
        broker.register(
            claimID: destinationClaimID,
            view: view,
            host: destinationHost,
            configuration: configuration("destination", recorder: recorder)
        )
        broker.update(
            claimID: sourceClaimID,
            view: view,
            host: sourceHost,
            configuration: configuration("restored-source", recorder: recorder)
        )

        broker.release(claimID: destinationClaimID, host: destinationHost)

        #expect(view.superview === sourceHost)
        #expect(sourceHost.subviews == [view])
        #expect(destinationHost.subviews.isEmpty)
        #expect(recorder.prepared.last == "restored-source")
        #expect(recorder.mounted.last?.0 == "restored-source")
        #expect(recorder.mounted.last?.1 == true)
        #expect(recorder.unmounted.isEmpty)
    }

    @Test("ineligible claims cannot displace the current presentation")
    func ineligibleClaimDoesNotDisplaceCurrentPresentation() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let currentClaimID = UUID()
        let staleClaimID = UUID()
        let currentHost = ReparentingNSViewHost()
        let staleHost = ReparentingNSViewHost()
        let view = NSView()

        broker.register(
            claimID: currentClaimID,
            view: view,
            host: currentHost,
            configuration: configuration("current", recorder: recorder)
        )
        broker.register(
            claimID: staleClaimID,
            view: view,
            host: staleHost,
            configuration: configuration("stale", isEligible: false, recorder: recorder)
        )

        #expect(view.superview === currentHost)
        #expect(staleHost.subviews.isEmpty)
        #expect(!recorder.prepared.contains("stale"))
    }

    @Test("releasing the final claim deactivates and detaches its view")
    func finalReleaseUnmountsView() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let claimID = UUID()
        let host = ReparentingNSViewHost()
        let view = NSView()

        broker.register(
            claimID: claimID,
            view: view,
            host: host,
            configuration: configuration("only", recorder: recorder)
        )
        broker.release(claimID: claimID, host: host)

        #expect(view.superview == nil)
        #expect(host.subviews.isEmpty)
        #expect(recorder.unmounted == [ObjectIdentifier(view)])
    }

    @Test("changing a claim to another view releases the old view")
    func changingClaimViewReleasesOldView() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let claimID = UUID()
        let host = ReparentingNSViewHost()
        let firstView = NSView()
        let secondView = NSView()

        broker.register(
            claimID: claimID,
            view: firstView,
            host: host,
            configuration: configuration("first", recorder: recorder)
        )
        broker.update(
            claimID: claimID,
            view: secondView,
            host: host,
            configuration: configuration("second", recorder: recorder)
        )

        #expect(firstView.superview == nil)
        #expect(secondView.superview === host)
        #expect(host.subviews == [secondView])
        #expect(recorder.unmounted == [ObjectIdentifier(firstView)])
    }

    @Test("stale dismantle cannot release a claim rebound to another host")
    func staleDismantleCannotReleaseReboundClaim() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let claimID = UUID()
        let staleHost = ReparentingNSViewHost()
        let currentHost = ReparentingNSViewHost()
        let view = NSView()

        broker.register(
            claimID: claimID,
            view: view,
            host: staleHost,
            configuration: configuration("stale", recorder: recorder)
        )
        broker.update(
            claimID: claimID,
            view: view,
            host: currentHost,
            configuration: configuration("current", recorder: recorder)
        )
        broker.release(claimID: claimID, host: staleHost)

        #expect(view.superview === currentHost)
        #expect(staleHost.subviews.isEmpty)
        #expect(currentHost.subviews == [view])
        #expect(recorder.unmounted.isEmpty)
    }

    @Test("fallback evaluates surviving eligibility without a prior update")
    func fallbackUsesLiveEligibility() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let sourceClaimID = UUID()
        let destinationClaimID = UUID()
        let sourceHost = ReparentingNSViewHost()
        let destinationHost = ReparentingNSViewHost()
        let view = NSView()
        var sourceEligible = false

        broker.register(
            claimID: sourceClaimID,
            view: view,
            host: sourceHost,
            configuration: .init(
                isEligible: { sourceEligible },
                prepare: { _ in recorder.prepared.append("source") },
                didMount: { _, _, changed in recorder.mounted.append(("source", changed)) }
            )
        )
        broker.register(
            claimID: destinationClaimID,
            view: view,
            host: destinationHost,
            configuration: configuration("destination", recorder: recorder)
        )
        sourceEligible = true

        broker.release(claimID: destinationClaimID, host: destinationHost)

        #expect(view.superview === sourceHost)
        #expect(sourceHost.subviews == [view])
        #expect(recorder.prepared.last == "source")
        #expect(recorder.unmounted.isEmpty)
    }

    @Test("host resizes only its broker-owned view")
    func hostResizesBrokerOwnedView() {
        let recorder = Recorder()
        let broker = makeBroker(recorder: recorder)
        let host = ReparentingNSViewHost(frame: NSRect(x: 0, y: 0, width: 320, height: 180))
        let view = NSView()

        broker.register(
            claimID: UUID(),
            view: view,
            host: host,
            configuration: configuration("view", recorder: recorder)
        )
        host.frame.size = NSSize(width: 640, height: 360)
        host.layout()

        #expect(view.frame == host.bounds)
    }

    private func makeBroker(
        recorder: Recorder
    ) -> ReparentingNSViewBroker<NSView> {
        ReparentingNSViewBroker { view in
            recorder.unmounted.append(ObjectIdentifier(view))
        }
    }

    private func configuration(
        _ name: String,
        isEligible: Bool = true,
        recorder: Recorder
    ) -> ReparentingNSViewBroker<NSView>.Configuration {
        .init(
            isEligible: { isEligible },
            prepare: { _ in
                recorder.prepared.append(name)
            },
            didMount: { _, _, changed in
                recorder.mounted.append((name, changed))
            }
        )
    }
}
