import Foundation
import Testing

@testable import Muxy

@Suite("TerminalViewRegistry PID resolution")
struct TerminalViewRegistryPIDTests {
    @Test("resolves the pane matching the nearest known ancestor")
    func resolvesNearestKnownAncestor() {
        let shellPaneID = UUID()
        let agentPaneID = UUID()
        let identities = [
            TerminalViewRegistry.PaneProcessIdentity(paneID: shellPaneID, processID: 100),
            TerminalViewRegistry.PaneProcessIdentity(paneID: agentPaneID, processID: 200),
        ]

        let paneID = TerminalViewRegistry.resolvePaneID(
            processIDs: [300, 200, 100],
            identities: identities
        )

        #expect(paneID == agentPaneID)
    }

    @Test("ignores invalid process identities")
    func ignoresInvalidProcessIdentities() {
        let paneID = UUID()
        let identities = [
            TerminalViewRegistry.PaneProcessIdentity(paneID: paneID, processID: 100),
        ]

        let resolved = TerminalViewRegistry.resolvePaneID(
            processIDs: [0, -1, 100],
            identities: identities
        )

        #expect(resolved == paneID)
    }

    @Test("returns nil when no pane process matches")
    func returnsNilWithoutMatch() {
        let identities = [
            TerminalViewRegistry.PaneProcessIdentity(paneID: UUID(), processID: 100),
        ]

        let paneID = TerminalViewRegistry.resolvePaneID(
            processIDs: [300, 200],
            identities: identities
        )

        #expect(paneID == nil)
    }

    @Test("uses stable pane ordering when identities share a process")
    func usesStablePaneOrderingForSharedProcess() throws {
        let firstPaneID = try #require(UUID(uuidString: "00000000-0000-0000-0000-000000000001"))
        let secondPaneID = try #require(UUID(uuidString: "00000000-0000-0000-0000-000000000002"))
        let identities = [
            TerminalViewRegistry.PaneProcessIdentity(paneID: secondPaneID, processID: 100),
            TerminalViewRegistry.PaneProcessIdentity(paneID: firstPaneID, processID: 100),
        ]

        let paneID = TerminalViewRegistry.resolvePaneID(
            processIDs: [100],
            identities: identities
        )

        #expect(paneID == firstPaneID)
    }
}
