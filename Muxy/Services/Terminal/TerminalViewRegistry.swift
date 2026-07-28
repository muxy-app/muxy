import AppKit

@MainActor
final class TerminalViewRegistry {
    struct PaneProcessIdentity: Equatable, Sendable {
        let paneID: UUID
        let processID: Int32
    }

    static let shared = TerminalViewRegistry()

    private var views: [UUID: any TerminalSurface] = [:]
    private var paneIDs: [ObjectIdentifier: UUID] = [:]
    private var processIdentityOverride: [PaneProcessIdentity]?

    private init() {}

    func overrideProcessIdentities(_ identities: [PaneProcessIdentity]?) {
        processIdentityOverride = identities
    }

    func isOwnedByRemote(_ paneID: UUID) -> Bool {
        !PaneOwnershipStore.shared.isOwnedByMac(paneID)
    }

    func view(
        for paneID: UUID,
        workingDirectory: String,
        command: String? = nil,
        commandInteractive: Bool = false,
        closesOnCommandExit: Bool = true,
        workspaceContext: WorkspaceContext = .local,
        backend: TerminalBackend = .fallback
    ) -> any TerminalSurface {
        if let existing = views[paneID] {
            return existing
        }
        let view = backend.makeSurface(launch: TerminalLaunchRequest(
            workingDirectory: workingDirectory,
            command: command,
            commandInteractive: commandInteractive,
            closesOnCommandExit: closesOnCommandExit,
            workspaceContext: workspaceContext
        ))
        views[paneID] = view
        paneIDs[ObjectIdentifier(view.terminalView)] = paneID
        return view
    }

    func existingView(for paneID: UUID) -> (any TerminalSurface)? {
        views[paneID]
    }

    func removeView(for paneID: UUID) {
        guard let view = views.removeValue(forKey: paneID) else { return }
        TerminalCommandTracker.shared.removePane(paneID)
        view.tearDown()
        paneIDs.removeValue(forKey: ObjectIdentifier(view.terminalView))
    }

    func needsConfirmQuit(for paneID: UUID) -> Bool {
        views[paneID]?.needsConfirmQuit() ?? false
    }

    func view(for paneID: UUID) -> (any TerminalSurface)? {
        views[paneID]
    }

    func paneID(for view: any TerminalSurface) -> UUID? {
        paneIDs[ObjectIdentifier(view.terminalView)]
    }

    func surface(for terminalView: NSView) -> (any TerminalSurface)? {
        guard let paneID = paneIDs[ObjectIdentifier(terminalView)] else { return nil }
        return views[paneID]
    }

    func paneID(matchingProcessIDs processIDs: [Int32]) -> UUID? {
        let identities = processIdentityOverride ?? views.compactMap { entry -> PaneProcessIdentity? in
            let (paneID, view) = entry
            guard let processID = view.foregroundProcessID else { return nil }
            return PaneProcessIdentity(paneID: paneID, processID: processID)
        }
        return Self.resolvePaneID(processIDs: processIDs, identities: identities)
    }

    nonisolated static func resolvePaneID(
        processIDs: [Int32],
        identities: [PaneProcessIdentity]
    ) -> UUID? {
        let paneIDsByProcessID = Dictionary(grouping: identities, by: \.processID)
            .mapValues { matches in
                matches.map(\.paneID).sorted { $0.uuidString < $1.uuidString }
            }
        for processID in processIDs where processID > 0 {
            guard let paneID = paneIDsByProcessID[processID]?.first else { continue }
            return paneID
        }
        return nil
    }

    func applyColorSchemeToAllViews(isDark _: Bool) {
        for view in views.values {
            view.reapplyActiveColors()
        }
    }

    func reapplyClientThemes() {
        for view in views.values {
            (view as? any TerminalClientThemeSurface)?.reapplyClientThemeIfOwned()
        }
    }

    var liveViews: [any TerminalSurface] {
        Array(views.values)
    }

    var liveViewCount: Int {
        views.count
    }

    var liveSurfaceCount: Int {
        views.values.reduce(0) { $1.hasLiveSurface ? $0 + 1 : $0 }
    }
}

extension TerminalViewRegistry: TerminalViewRemoving {}
