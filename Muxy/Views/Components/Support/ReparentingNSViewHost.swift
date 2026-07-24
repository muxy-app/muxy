import AppKit

final class ReparentingNSViewHost: NSView {
    private weak var hostedView: NSView?

    fileprivate var ownedHostedView: NSView? {
        guard hostedView?.superview === self else { return nil }
        return hostedView
    }

    @discardableResult
    fileprivate func host(_ view: NSView) -> Bool {
        guard hostedView !== view || view.superview !== self else { return false }
        _ = detachHostedView()
        view.removeFromSuperview()
        view.translatesAutoresizingMaskIntoConstraints = true
        view.frame = bounds
        view.autoresizingMask = [.width, .height]
        addSubview(view)
        hostedView = view
        return true
    }

    @discardableResult
    fileprivate func detachHostedView() -> NSView? {
        let ownedView = ownedHostedView
        ownedView?.removeFromSuperview()
        hostedView = nil
        return ownedView
    }

    override func layout() {
        super.layout()
        guard let hostedView = ownedHostedView else { return }
        guard hostedView.frame != bounds else { return }
        hostedView.frame = bounds
    }
}

@MainActor
final class ReparentingNSViewBroker<View: NSView> {
    struct Configuration {
        let isEligible: () -> Bool
        let prepare: (View) -> Void
        let didMount: (View, ReparentingNSViewHost, Bool) -> Void
    }

    private final class Claim {
        let id: UUID
        let order: UInt64
        let view: View
        weak var host: ReparentingNSViewHost?
        var configuration: Configuration

        init(
            id: UUID,
            order: UInt64,
            view: View,
            host: ReparentingNSViewHost,
            configuration: Configuration
        ) {
            self.id = id
            self.order = order
            self.view = view
            self.host = host
            self.configuration = configuration
        }
    }

    private var claims: [UUID: Claim] = [:]
    private var claimIDsByView: [ObjectIdentifier: Set<UUID>] = [:]
    private var activeClaimIDs: [ObjectIdentifier: UUID] = [:]
    private var nextOrder: UInt64 = 0
    private let unmount: (View) -> Void

    init(unmount: @escaping (View) -> Void) {
        self.unmount = unmount
    }

    func register(
        claimID: UUID,
        view: View,
        host: ReparentingNSViewHost,
        configuration: Configuration
    ) {
        if claims[claimID] != nil {
            update(
                claimID: claimID,
                view: view,
                host: host,
                configuration: configuration
            )
            return
        }
        nextOrder &+= 1
        let claim = Claim(
            id: claimID,
            order: nextOrder,
            view: view,
            host: host,
            configuration: configuration
        )
        claims[claimID] = claim
        claimIDsByView[ObjectIdentifier(view), default: []].insert(claimID)
        reconcile(view)
    }

    func update(
        claimID: UUID,
        view: View,
        host: ReparentingNSViewHost,
        configuration: Configuration
    ) {
        guard let claim = claims[claimID] else {
            register(
                claimID: claimID,
                view: view,
                host: host,
                configuration: configuration
            )
            return
        }
        guard claim.view === view else {
            remove(claimID)
            register(
                claimID: claimID,
                view: view,
                host: host,
                configuration: configuration
            )
            return
        }
        claim.host = host
        claim.configuration = configuration
        reconcile(view)
    }

    @discardableResult
    func release(
        claimID: UUID,
        host: ReparentingNSViewHost
    ) -> Bool {
        guard claims[claimID]?.host === host else { return false }
        remove(claimID)
        return true
    }

    private func remove(_ claimID: UUID) {
        guard let claim = claims.removeValue(forKey: claimID) else { return }
        let viewID = ObjectIdentifier(claim.view)
        claimIDsByView[viewID]?.remove(claimID)
        if claimIDsByView[viewID]?.isEmpty == true {
            claimIDsByView.removeValue(forKey: viewID)
        }
        reconcile(claim.view)
    }

    private func reconcile(_ view: View) {
        let viewID = ObjectIdentifier(view)
        let registeredIDs = claimIDsByView[viewID] ?? []
        let liveClaims = registeredIDs.compactMap { claimID -> Claim? in
            guard let claim = claims[claimID] else { return nil }
            guard claim.host != nil else {
                claims.removeValue(forKey: claimID)
                claimIDsByView[viewID]?.remove(claimID)
                return nil
            }
            return claim
        }
        if claimIDsByView[viewID]?.isEmpty == true {
            claimIDsByView.removeValue(forKey: viewID)
        }
        guard let activeClaim = liveClaims
            .filter({ $0.configuration.isEligible() })
            .max(by: { $0.order < $1.order }),
            let activeHost = activeClaim.host
        else {
            let hadActiveClaim = activeClaimIDs.removeValue(forKey: viewID) != nil
            guard hadActiveClaim || view.superview != nil else { return }
            unmount(view)
            if let host = view.superview as? ReparentingNSViewHost {
                _ = host.detachHostedView()
            } else {
                view.removeFromSuperview()
            }
            return
        }

        let ownershipChanged = activeClaimIDs[viewID] != activeClaim.id
        activeClaimIDs[viewID] = activeClaim.id
        activeClaim.configuration.prepare(view)
        let attachmentChanged = activeHost.host(view)
        activeClaim.configuration.didMount(
            view,
            activeHost,
            ownershipChanged || attachmentChanged
        )
    }
}
