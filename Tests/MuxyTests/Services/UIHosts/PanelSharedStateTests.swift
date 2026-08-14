import AppKit
import Foundation
import SwiftUI
import Testing

@testable import Muxy

@Suite("Panel shared state", .serialized)
struct PanelSharedStateTests {
    @Suite("PanelHost")
    @MainActor
    struct PanelHostTests {
        private func makeHost() -> PanelHost {
            let host = PanelHost.shared
            host.closeAll()
            return host
        }

        @Test("opening a panel records its placement")
        func opensPanel() {
            let host = makeHost()
            host.open("a", at: .right, mode: .pinned)
            #expect(host.isOpen("a"))
            #expect(host.pinnedPanel(at: .right) == "a")
        }

        @Test("only one pinned panel per position")
        func onePinnedPerPosition() {
            let host = makeHost()
            host.open("a", at: .right, mode: .pinned)
            host.open("b", at: .right, mode: .pinned)
            #expect(host.pinnedPanel(at: .right) == "b")
            #expect(!host.isOpen("a"))
        }

        @Test("only one floating panel per position")
        func oneFloatingPerPosition() {
            let host = makeHost()
            host.open("a", at: .bottom, mode: .floating)
            host.open("b", at: .bottom, mode: .floating)
            #expect(host.floatingPanel(at: .bottom) == "b")
            #expect(!host.isOpen("a"))
        }

        @Test("pinned and floating coexist at the same position")
        func pinnedAndFloatingCoexist() {
            let host = makeHost()
            host.open("pinned", at: .right, mode: .pinned)
            host.open("floating", at: .right, mode: .floating)
            #expect(host.pinnedPanel(at: .right) == "pinned")
            #expect(host.floatingPanel(at: .right) == "floating")
        }

        @Test("a panel opened twice keeps a single placement")
        func reopenMovesPanel() {
            let host = makeHost()
            host.open("a", at: .right, mode: .pinned)
            host.open("a", at: .bottom, mode: .floating)
            #expect(host.pinnedPanel(at: .right) == nil)
            #expect(host.floatingPanel(at: .bottom) == "a")
            #expect(host.placements.count == 1)
        }

        @Test("toggle opens then closes the same panel")
        func toggle() {
            let host = makeHost()
            host.toggle("a", at: .right, mode: .pinned)
            #expect(host.isOpen("a"))
            host.toggle("a", at: .right, mode: .pinned)
            #expect(!host.isOpen("a"))
        }

        @Test("move preserves mode")
        func movePreservesMode() {
            let host = makeHost()
            host.open("a", at: .right, mode: .floating)
            host.move("a", to: .bottom)
            #expect(host.placement(for: "a")?.position == .bottom)
            #expect(host.placement(for: "a")?.mode == .floating)
        }

        @Test("setMode preserves position and displaces same-mode panel")
        func setMode() {
            let host = makeHost()
            host.open("a", at: .right, mode: .pinned)
            host.open("b", at: .right, mode: .floating)
            host.setMode(.pinned, for: "b")
            #expect(host.placement(for: "b")?.mode == .pinned)
            #expect(host.placement(for: "b")?.position == .right)
            #expect(!host.isOpen("a"))
        }

        @Test("opening over an occupied slot reports the displaced panel")
        func displaceNotifiesEvictedPanel() {
            let host = makeHost()
            let previous = host.onDisplace
            defer { host.onDisplace = previous }
            var displaced: [String] = []
            host.onDisplace = { displaced.append($0) }

            host.open("a", at: .right, mode: .floating)
            host.open("b", at: .right, mode: .floating)
            #expect(displaced == ["a"])

            host.move("b", to: .right)
            #expect(displaced == ["a"])
        }
    }

    @Suite("ExtensionPanelRegistry")
    @MainActor
    struct ExtensionPanelRegistryTests {
        @Test("displacing a panel at the same slot emits panel.closed for the displaced panel")
        func displacementEmitsPanelClosed() async {
            let registry = ExtensionPanelRegistry.shared
            registry.closeAll(extensionID: "ext-a")
            registry.closeAll(extensionID: "ext-b")

            let collector = EventCollector()
            let token = NotificationSocketServer.shared.addInProcessObserver { collector.add($0) }
            defer { NotificationSocketServer.shared.removeInProcessObserver(token) }

            registry.open(extensionID: "ext-a", panel: panel(id: "first"), data: nil)
            registry.open(extensionID: "ext-b", panel: panel(id: "second"), data: nil)
            defer { registry.closeAll(extensionID: "ext-b") }

            let delivered = await waitFor(timeout: 2.0) {
                collector.closedPanelIDs(extensionID: "ext-a").contains("first")
            }
            #expect(delivered)
            #expect(!collector.closedPanelIDs(extensionID: "ext-b").contains("second"))
        }

        @Test("closing a focused extension panel restores its previous responder")
        func closeRestoresPreviousResponder() {
            let extensionID = "focus-restoration-\(UUID().uuidString)"
            let registry = ExtensionPanelRegistry.shared
            let state = registry.open(
                extensionID: extensionID,
                panel: panel(id: "files"),
                data: nil
            )
            defer { registry.closeAll(extensionID: extensionID) }
            let window = focusTestWindow()
            let previousResponder = FocusTestView()
            let panelView = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(panelView)
            #expect(window.makeFirstResponder(previousResponder))

            PanelFocusRestoration.shared.captureBeforeClaim(
                panelID: state.hostPanelID,
                panelView: panelView
            )
            #expect(window.makeFirstResponder(panelView))

            registry.forceClose(hostPanelID: state.hostPanelID)

            #expect(window.firstResponder === previousResponder)
        }

        @Test("closing all panels restores focus through out-of-order snapshots")
        func closeAllRestoresOriginalResponder() {
            let extensionID = "focus-restoration-\(UUID().uuidString)"
            let registry = ExtensionPanelRegistry.shared
            let firstState = registry.open(
                extensionID: extensionID,
                panel: panel(id: "first"),
                data: nil
            )
            let secondState = registry.open(
                extensionID: extensionID,
                panel: panel(id: "second", position: .bottom),
                data: nil
            )
            defer { registry.closeAll(extensionID: extensionID) }
            let window = focusTestWindow()
            let previousResponder = FocusTestView()
            let firstPanelView = FocusTestView()
            let secondPanelView = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(firstPanelView)
            window.contentView?.addSubview(secondPanelView)
            #expect(window.makeFirstResponder(previousResponder))

            PanelFocusRestoration.shared.captureBeforeClaim(
                panelID: firstState.hostPanelID,
                panelView: firstPanelView
            )
            #expect(window.makeFirstResponder(firstPanelView))
            PanelFocusRestoration.shared.captureBeforeClaim(
                panelID: secondState.hostPanelID,
                panelView: secondPanelView
            )
            #expect(window.makeFirstResponder(secondPanelView))

            registry.closeAll(extensionID: extensionID)

            #expect(window.firstResponder === previousResponder)
        }

        @Test("an allowed stale close verdict preserves a replacement panel")
        func staleCloseVerdictPreservesReplacement() async throws {
            let extensionID = "stale-close-\(UUID().uuidString)"
            let registry = ExtensionPanelRegistry.shared
            let originalState = registry.open(
                extensionID: extensionID,
                panel: panel(id: "files"),
                data: nil
            )
            let surfaceKey = LifecycleSurfaceKey(
                kind: .panel,
                instanceID: originalState.id.uuidString
            )
            let bridge = DeferredPanelBeforeCloseAsking()
            ExtensionSurfaceBridgeRegistry.shared.register(bridge, for: surfaceKey)
            defer {
                ExtensionSurfaceBridgeRegistry.shared.unregister(surfaceKey, ifMatches: bridge)
                registry.closeAll(extensionID: extensionID)
            }

            registry.close(hostPanelID: originalState.hostPanelID)
            let requested = await waitFor(timeout: 1) {
                bridge.askCount == 1
            }
            #expect(requested)

            let replacementState = registry.open(
                extensionID: extensionID,
                panel: panel(id: "files"),
                data: nil
            )
            bridge.resolve(.allow)
            let completed = await waitFor(timeout: 1) {
                bridge.completedRequestCount == 1
            }
            #expect(completed)

            let currentState = try #require(registry.state(forHostPanelID: originalState.hostPanelID))
            #expect(currentState.id == replacementState.id)
            #expect(PanelHost.shared.isOpen(replacementState.hostPanelID))
        }

        @Test("project activation restores each project's open panels")
        func projectActivationRestoresPanels() {
            let registry = ExtensionPanelRegistry.shared
            let projectA = UUID()
            let projectB = UUID()
            let filesExtension = "files-\(UUID().uuidString)"
            let gitExtension = "git-\(UUID().uuidString)"
            defer {
                registry.closeAll(extensionID: filesExtension)
                registry.closeAll(extensionID: gitExtension)
                registry.activateProject(nil, from: registry.activeProjectID)
            }

            registry.activateProject(projectA, from: nil)
            registry.open(
                extensionID: filesExtension,
                panel: panel(id: "browser", mode: .pinned),
                data: .object(["path": .string("/a")])
            )
            registry.move(.bottom, forHostPanelID: ExtensionPanelState.hostPanelID(
                extensionID: filesExtension,
                panelID: "browser"
            ))

            registry.activateProject(projectB, from: projectA)
            #expect(registry.openStates.isEmpty)
            #expect(!PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
                extensionID: filesExtension,
                panelID: "browser"
            )))

            registry.open(
                extensionID: gitExtension,
                panel: panel(id: "changes", position: .right, mode: .floating),
                data: nil
            )
            #expect(PanelHost.shared.placement(for: ExtensionPanelState.hostPanelID(
                extensionID: gitExtension,
                panelID: "changes"
            ))?.mode == .floating)

            registry.activateProject(projectA, from: projectB)
            let filesHost = ExtensionPanelState.hostPanelID(
                extensionID: filesExtension,
                panelID: "browser"
            )
            #expect(registry.state(forHostPanelID: filesHost) != nil)
            #expect(PanelHost.shared.placement(for: filesHost)?.position == .bottom)
            #expect(PanelHost.shared.placement(for: filesHost)?.mode == .pinned)
            #expect(registry.state(forHostPanelID: filesHost)?.initialData == .object(["path": .string("/a")]))
            #expect(!PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
                extensionID: gitExtension,
                panelID: "changes"
            )))

            registry.activateProject(projectB, from: projectA)
            #expect(PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
                extensionID: gitExtension,
                panelID: "changes"
            )))
            #expect(!PanelHost.shared.isOpen(filesHost))
        }

        @Test("project activation leaves the builtin console open")
        func projectActivationPreservesBuiltinConsole() {
            let registry = ExtensionPanelRegistry.shared
            let projectA = UUID()
            let projectB = UUID()
            let extensionID = "console-coexist-\(UUID().uuidString)"
            PanelHost.shared.open(BuiltinPanel.extensionConsole, at: .bottom, mode: .floating)
            defer {
                PanelHost.shared.close(BuiltinPanel.extensionConsole)
                registry.closeAll(extensionID: extensionID)
                registry.activateProject(nil, from: registry.activeProjectID)
            }

            registry.activateProject(projectA, from: nil)
            registry.open(extensionID: extensionID, panel: panel(id: "side"), data: nil)
            registry.activateProject(projectB, from: projectA)

            #expect(PanelHost.shared.isOpen(BuiltinPanel.extensionConsole))
            #expect(registry.openStates.isEmpty)
        }

        @Test("project restore skips snapshots that would displace the builtin console")
        func projectRestoreSkipsConsoleSlotConflict() {
            let registry = ExtensionPanelRegistry.shared
            let projectA = UUID()
            let projectB = UUID()
            let extensionID = "console-slot-\(UUID().uuidString)"
            defer {
                PanelHost.shared.close(BuiltinPanel.extensionConsole)
                registry.closeAll(extensionID: extensionID)
                registry.activateProject(nil, from: registry.activeProjectID)
            }

            registry.activateProject(projectA, from: nil)
            registry.open(
                extensionID: extensionID,
                panel: panel(id: "bottom", position: .bottom, mode: .floating),
                data: nil
            )
            let hostPanelID = ExtensionPanelState.hostPanelID(
                extensionID: extensionID,
                panelID: "bottom"
            )
            #expect(PanelHost.shared.isOpen(hostPanelID))

            registry.activateProject(projectB, from: projectA)
            PanelHost.shared.open(BuiltinPanel.extensionConsole, at: .bottom, mode: .floating)
            registry.activateProject(projectA, from: projectB)

            #expect(PanelHost.shared.isOpen(BuiltinPanel.extensionConsole))
            #expect(!PanelHost.shared.isOpen(hostPanelID))
            #expect(registry.openStates.isEmpty)
        }

        @Test("project restore checks console conflicts against the persisted panel mode")
        func projectRestoreChecksPersistedModeForConsoleConflict() throws {
            let suiteName = "PanelSharedStateTests-\(UUID().uuidString)"
            let defaults = try #require(UserDefaults(suiteName: suiteName))
            defer { defaults.removePersistentDomain(forName: suiteName) }
            let host = PanelHost(modePreferences: PanelModePreferences(defaults: defaults))
            let registry = ExtensionPanelRegistry(panelHost: host)
            let projectA = UUID()
            let projectB = UUID()
            let extensionID = "persisted-console-slot-\(UUID().uuidString)"
            let definedPanel = panel(id: "bottom", position: .bottom, mode: .floating)
            let hostPanelID = ExtensionPanelState.hostPanelID(
                extensionID: extensionID,
                panelID: definedPanel.id
            )

            registry.activateProject(projectA, from: nil)
            registry.open(extensionID: extensionID, panel: definedPanel, data: nil)
            registry.activateProject(projectB, from: projectA)
            registry.open(extensionID: extensionID, panel: definedPanel, data: nil)
            registry.setMode(.pinned, forHostPanelID: hostPanelID)
            registry.forceClose(hostPanelID: hostPanelID)
            host.open(
                BuiltinPanel.extensionConsole,
                at: .bottom,
                mode: .pinned,
                usesPreferredMode: false
            )

            registry.activateProject(projectA, from: projectB)

            #expect(host.isOpen(BuiltinPanel.extensionConsole))
            #expect(!host.isOpen(hostPanelID))
            #expect(registry.openStates.isEmpty)

            registry.activateProject(projectB, from: projectA)
            host.close(BuiltinPanel.extensionConsole)
            registry.activateProject(projectA, from: projectB)

            #expect(host.placement(for: hostPanelID)?.mode == .pinned)
        }

        @Test("a snapshot blocked by the console is restored once the console closes")
        func projectRestoreRetainsConsoleBlockedSnapshot() {
            let registry = ExtensionPanelRegistry.shared
            let projectA = UUID()
            let projectB = UUID()
            let extensionID = "console-deferred-\(UUID().uuidString)"
            defer {
                PanelHost.shared.close(BuiltinPanel.extensionConsole)
                registry.closeAll(extensionID: extensionID)
                registry.activateProject(nil, from: registry.activeProjectID)
            }

            registry.activateProject(projectA, from: nil)
            registry.open(
                extensionID: extensionID,
                panel: panel(id: "bottom", position: .bottom, mode: .floating),
                data: nil
            )
            let hostPanelID = ExtensionPanelState.hostPanelID(
                extensionID: extensionID,
                panelID: "bottom"
            )

            registry.activateProject(projectB, from: projectA)
            PanelHost.shared.open(BuiltinPanel.extensionConsole, at: .bottom, mode: .floating)
            registry.activateProject(projectA, from: projectB)
            #expect(!PanelHost.shared.isOpen(hostPanelID))

            registry.activateProject(projectB, from: projectA)
            PanelHost.shared.close(BuiltinPanel.extensionConsole)
            registry.activateProject(projectA, from: projectB)

            #expect(PanelHost.shared.isOpen(hostPanelID))
            #expect(PanelHost.shared.placement(for: hostPanelID)?.position == .bottom)
            #expect(PanelHost.shared.placement(for: hostPanelID)?.mode == .floating)
        }

        @Test("captureLiveSnapshots preserves panel entry and chrome fields")
        func captureLiveSnapshotsPreservesPanelDefinition() {
            let registry = ExtensionPanelRegistry.shared
            let extensionID = "snapshot-chrome-\(UUID().uuidString)"
            let defined = ExtensionPanel(
                id: "review",
                title: "Review",
                icon: .symbol("checklist"),
                entry: "panels/review.html",
                position: .right,
                mode: .floating,
                hiddenControls: [.pin],
                hideTopbar: false,
                defaultData: .object(["mode": .string("diff")])
            )
            defer {
                registry.closeAll(extensionID: extensionID)
                registry.activateProject(nil, from: registry.activeProjectID)
            }

            registry.activateProject(UUID(), from: nil)
            registry.open(extensionID: extensionID, panel: defined, data: .object(["mode": .string("diff")]))
            let snapshots = registry.captureLiveSnapshots()
            let snapshot = snapshots.first { $0.extensionID == extensionID && $0.panelID == "review" }

            #expect(snapshot?.entry == "panels/review.html")
            #expect(snapshot?.title == "Review")
            #expect(snapshot?.icon == .symbol("checklist"))
            #expect(snapshot?.hiddenControls == [.pin])
            #expect(snapshot?.initialData == .object(["mode": .string("diff")]))
        }

        @Test("closeAll purges inactive project snapshots for that extension")
        func closeAllPurgesInactiveSnapshots() {
            let registry = ExtensionPanelRegistry.shared
            let projectA = UUID()
            let projectB = UUID()
            let extensionID = "purge-\(UUID().uuidString)"
            defer {
                registry.closeAll(extensionID: extensionID)
                registry.activateProject(nil, from: registry.activeProjectID)
            }

            registry.activateProject(projectA, from: nil)
            registry.open(extensionID: extensionID, panel: panel(id: "files"), data: nil)
            registry.activateProject(projectB, from: projectA)
            registry.closeAll(extensionID: extensionID)
            registry.activateProject(projectA, from: projectB)

            #expect(registry.openStates.isEmpty)
            #expect(!PanelHost.shared.isOpen(ExtensionPanelState.hostPanelID(
                extensionID: extensionID,
                panelID: "files"
            )))
        }

        @Test("purgeProject drops snapshots and live panels for that project")
        func purgeProjectDropsState() {
            let registry = ExtensionPanelRegistry.shared
            let projectA = UUID()
            let projectB = UUID()
            let extensionID = "purge-project-\(UUID().uuidString)"
            defer {
                registry.closeAll(extensionID: extensionID)
                registry.activateProject(nil, from: registry.activeProjectID)
            }

            registry.activateProject(projectA, from: nil)
            registry.open(extensionID: extensionID, panel: panel(id: "files"), data: nil)
            registry.activateProject(projectB, from: projectA)
            registry.purgeProject(projectA)
            registry.activateProject(projectA, from: projectB)

            #expect(registry.openStates.isEmpty)
        }

        private func panel(
            id: String,
            position: PanelPosition = .right,
            mode: PanelMode = .pinned
        ) -> ExtensionPanel {
            ExtensionPanel(id: id, entry: "index.html", position: position, mode: mode)
        }

        private func waitFor(timeout: TimeInterval, condition: () -> Bool) async -> Bool {
            let deadline = Date().addingTimeInterval(timeout)
            while Date() < deadline {
                if condition() { return true }
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            return condition()
        }
    }

    @Suite("PanelFocusRestoration")
    @MainActor
    struct PanelFocusRestorationTests {
        @Test("replacement panel views preserve the original responder")
        func replacementViewsPreserveOriginalResponder() {
            let restoration = PanelFocusRestoration()
            let window = focusTestWindow()
            let previousResponder = FocusTestView()
            let firstPanelView = FocusTestView()
            let replacementPanelView = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(firstPanelView)
            window.contentView?.addSubview(replacementPanelView)
            #expect(window.makeFirstResponder(previousResponder))

            restoration.captureBeforeClaim(panelID: "files", panelView: firstPanelView)
            #expect(window.makeFirstResponder(firstPanelView))
            restoration.captureBeforeClaim(panelID: "files", panelView: replacementPanelView)
            #expect(window.makeFirstResponder(replacementPanelView))

            restoration.restoreAfterClosing(panelID: "files")

            #expect(window.firstResponder === previousResponder)
        }

        @Test("closing a panel does not override focus moved elsewhere")
        func newerFocusIsPreserved() {
            let restoration = PanelFocusRestoration()
            let window = focusTestWindow()
            let previousResponder = FocusTestView()
            let panelView = FocusTestView()
            let panelControl = FocusTestView()
            let newerResponder = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(panelView)
            window.contentView?.addSubview(panelControl)
            window.contentView?.addSubview(newerResponder)
            #expect(window.makeFirstResponder(previousResponder))

            restoration.captureBeforeClaim(panelID: "files", panelView: panelView)
            #expect(window.makeFirstResponder(panelView))
            #expect(window.makeFirstResponder(panelControl))
            restoration.setPanelControlFocused(panelID: "files", focused: true)
            #expect(window.makeFirstResponder(newerResponder))
            restoration.setPanelControlFocused(panelID: "files", focused: false)

            restoration.restoreAfterClosing(panelID: "files")

            #expect(window.firstResponder === newerResponder)
        }

        @Test("closing from a focused panel control restores the original responder")
        func focusedPanelControlRestoresOriginalResponder() {
            let restoration = PanelFocusRestoration()
            let window = focusTestWindow()
            let previousResponder = FocusTestView()
            let panelView = FocusTestView()
            let panelControl = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(panelView)
            window.contentView?.addSubview(panelControl)
            #expect(window.makeFirstResponder(previousResponder))

            restoration.captureBeforeClaim(panelID: "files", panelView: panelView)
            #expect(window.makeFirstResponder(panelView))
            #expect(window.makeFirstResponder(panelControl))
            restoration.setPanelControlFocused(panelID: "files", focused: true)

            restoration.restoreAfterClosing(panelID: "files")

            #expect(window.firstResponder === previousResponder)
        }

        @Test("closing an earlier panel rebases a later panel snapshot")
        func outOfOrderCloseRestoresOriginalResponder() {
            let restoration = PanelFocusRestoration()
            let window = focusTestWindow()
            let previousResponder = FocusTestView()
            let firstPanelView = FocusTestView()
            let secondPanelView = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(firstPanelView)
            window.contentView?.addSubview(secondPanelView)
            #expect(window.makeFirstResponder(previousResponder))

            restoration.captureBeforeClaim(panelID: "first", panelView: firstPanelView)
            #expect(window.makeFirstResponder(firstPanelView))
            restoration.captureBeforeClaim(panelID: "second", panelView: secondPanelView)
            #expect(window.makeFirstResponder(secondPanelView))

            restoration.restoreAfterClosing(panelID: "first")
            #expect(window.firstResponder === secondPanelView)
            restoration.restoreAfterClosing(panelID: "second")

            #expect(window.firstResponder === previousResponder)
        }

        @Test("replacement survives dependent closure after the old view is released")
        func replacementSurvivesDependentClosureAfterOldViewRelease() {
            let restoration = PanelFocusRestoration()
            let window = focusTestWindow()
            window.autorecalculatesKeyViewLoop = false
            let previousResponder = FocusTestView()
            let secondPanelView = FocusTestView()
            let thirdPanelView = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(secondPanelView)
            window.contentView?.addSubview(thirdPanelView)
            #expect(window.makeFirstResponder(previousResponder))

            weak var releasedFirstPanelView: FocusTestView?
            autoreleasepool {
                let firstPanelView = FocusTestView()
                let movedFirstPanelView = FocusTestView()
                releasedFirstPanelView = firstPanelView
                window.contentView?.addSubview(firstPanelView)
                window.contentView?.addSubview(movedFirstPanelView)

                restoration.captureBeforeClaim(panelID: "first", panelView: firstPanelView)
                #expect(window.makeFirstResponder(firstPanelView))
                restoration.captureBeforeClaim(panelID: "second", panelView: secondPanelView)
                #expect(window.makeFirstResponder(secondPanelView))
                restoration.captureBeforeClaim(panelID: "third", panelView: thirdPanelView)
                #expect(window.makeFirstResponder(thirdPanelView))
                restoration.captureBeforeClaim(panelID: "first", panelView: movedFirstPanelView)
                #expect(window.makeFirstResponder(movedFirstPanelView))

                firstPanelView.removeFromSuperview()
            }
            #expect(releasedFirstPanelView == nil)

            #expect(window.makeFirstResponder(thirdPanelView))
            restoration.restoreAfterClosing(panelID: "second")
            #expect(window.firstResponder === thirdPanelView)
            restoration.restoreAfterClosing(panelID: "first")
            #expect(window.firstResponder === thirdPanelView)
            restoration.restoreAfterClosing(panelID: "third")

            #expect(window.firstResponder === previousResponder)
        }

        @Test("discard does not restore the previous responder")
        func discardDoesNotRestoreFocus() {
            let restoration = PanelFocusRestoration()
            let window = focusTestWindow()
            let previousResponder = FocusTestView()
            let panelView = FocusTestView()
            window.contentView?.addSubview(previousResponder)
            window.contentView?.addSubview(panelView)
            #expect(window.makeFirstResponder(previousResponder))

            restoration.captureBeforeClaim(panelID: "files", panelView: panelView)
            #expect(window.makeFirstResponder(panelView))

            restoration.discard(panelID: "files")

            #expect(window.firstResponder === panelView)
            #expect(window.firstResponder !== previousResponder)
        }
    }

    @Suite("PanelHostSlot")
    @MainActor
    struct PanelHostSlotTests {
        @Test("panel placement changes do not update the workspace subtree")
        func placementChangesDoNotUpdateWorkspace() async {
            let host = PanelHost.shared
            host.closeAll()
            defer { host.closeAll() }
            let workspaceRecorder = WorkspaceLifecycleRecorder()
            let panelRecorder = PanelContentLifecycleRecorder()
            let hostingView = NSHostingView(rootView: PanelHostSlotHarness(
                panelHost: host,
                workspaceRecorder: workspaceRecorder,
                panelRecorder: panelRecorder
            ))
            hostingView.frame = NSRect(x: 0, y: 0, width: 900, height: 600)
            hostingView.layoutSubtreeIfNeeded()
            await settle(hostingView)

            let initialMakeCount = workspaceRecorder.makeCount
            let initialUpdateCount = workspaceRecorder.updateCount
            #expect(panelRecorder.makeCount == 0)

            host.open("ext:files:browser", at: .right, mode: .pinned)
            let panelAppeared = await waitFor(hostingView) {
                panelRecorder.makeCount == 1
            }
            #expect(panelAppeared)

            host.close("ext:files:browser")
            let panelDisappeared = await waitFor(hostingView) {
                panelRecorder.dismantleCount == 1
            }
            #expect(panelDisappeared)

            #expect(initialMakeCount == 1)
            #expect(workspaceRecorder.makeCount == initialMakeCount)
            #expect(workspaceRecorder.updateCount == initialUpdateCount)
            #expect(workspaceRecorder.dismantleCount == 0)
        }

        private func waitFor(
            _ hostingView: NSHostingView<PanelHostSlotHarness>,
            condition: () -> Bool
        ) async -> Bool {
            let deadline = Date().addingTimeInterval(1)
            while Date() < deadline {
                if condition() { return true }
                await settle(hostingView)
            }
            return condition()
        }

        private func settle(_ hostingView: NSHostingView<PanelHostSlotHarness>) async {
            await Task.yield()
            try? await Task.sleep(for: .milliseconds(20))
            hostingView.layoutSubtreeIfNeeded()
        }
    }
}

@MainActor
private func focusTestWindow() -> NSWindow {
    let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
        styleMask: [.titled],
        backing: .buffered,
        defer: false
    )
    window.contentView = NSView(frame: window.contentLayoutRect)
    return window
}

@MainActor
private final class FocusTestView: NSView {
    override var acceptsFirstResponder: Bool { true }
}

private final class EventCollector: @unchecked Sendable {
    private let lock = NSLock()
    private var events: [ExtensionEvent] = []

    func add(_ event: ExtensionEvent) {
        lock.lock()
        events.append(event)
        lock.unlock()
    }

    func closedPanelIDs(extensionID: String) -> [String] {
        lock.lock()
        defer { lock.unlock() }
        return events
            .filter { $0.name == ExtensionEventName.panelClosed && $0.payload["extensionID"] == extensionID }
            .compactMap { $0.payload["panelID"] }
    }
}

@MainActor
private final class DeferredPanelBeforeCloseAsking: BeforeCloseAsking {
    private(set) var askCount = 0
    private(set) var completedRequestCount = 0
    private var continuation: CheckedContinuation<LifecycleVerdict, Never>?

    func requestBeforeClose(reason _: LifecycleSurfaceKind, instanceID _: String) async -> LifecycleVerdict {
        askCount += 1
        let verdict = await withCheckedContinuation { continuation = $0 }
        completedRequestCount += 1
        return verdict
    }

    func resolve(_ verdict: LifecycleVerdict) {
        let continuation = continuation
        self.continuation = nil
        continuation?.resume(returning: verdict)
    }

    func failPendingLifecycle() {
        resolve(.allow)
    }
}

@MainActor
private final class WorkspaceLifecycleRecorder {
    var makeCount = 0
    var updateCount = 0
    var dismantleCount = 0
}

@MainActor
private final class PanelContentLifecycleRecorder {
    var makeCount = 0
    var dismantleCount = 0
}

private struct PanelHostSlotHarness: View {
    let panelHost: PanelHost
    let workspaceRecorder: WorkspaceLifecycleRecorder
    let panelRecorder: PanelContentLifecycleRecorder

    var body: some View {
        HStack(spacing: 0) {
            WorkspaceLifecycleProbe(recorder: workspaceRecorder)
            PanelHostSlot(panelHost: panelHost, position: .right, mode: .pinned) { _ in
                PanelContentLifecycleProbe(recorder: panelRecorder)
                    .frame(width: 240)
            }
        }
    }
}

private struct WorkspaceLifecycleProbe: NSViewRepresentable {
    let recorder: WorkspaceLifecycleRecorder

    func makeCoordinator() -> WorkspaceLifecycleRecorder {
        recorder
    }

    func makeNSView(context _: Context) -> NSView {
        recorder.makeCount += 1
        return NSView()
    }

    func updateNSView(_: NSView, context _: Context) {
        recorder.updateCount += 1
    }

    static func dismantleNSView(_: NSView, coordinator: WorkspaceLifecycleRecorder) {
        coordinator.dismantleCount += 1
    }
}

private struct PanelContentLifecycleProbe: NSViewRepresentable {
    let recorder: PanelContentLifecycleRecorder

    func makeCoordinator() -> PanelContentLifecycleRecorder {
        recorder
    }

    func makeNSView(context _: Context) -> NSView {
        recorder.makeCount += 1
        return NSView()
    }

    func updateNSView(_: NSView, context _: Context) {}

    static func dismantleNSView(_: NSView, coordinator: PanelContentLifecycleRecorder) {
        coordinator.dismantleCount += 1
    }
}
