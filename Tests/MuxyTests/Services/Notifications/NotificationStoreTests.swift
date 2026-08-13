import Foundation
import Testing
import MuxyShared

@testable import Muxy

@Suite("NotificationStore", .serialized)
@MainActor
struct NotificationStoreTests {
    @Test("coalesces desktop delivery for a matching AI hook and OSC pair")
    func coalescesDesktopDeliveryForMatchingAIHookAndOSCPair() {
        let store = NotificationStore.shared
        store.clear()
        let desktopNotifier = NotificationDesktopNotifierSpy()
        let restoreDesktopDelivery = enableDesktopDelivery(store, notifier: desktopNotifier)
        defer { restoreDesktopDelivery() }

        let context = makeContext()
        let appState = makeAppState()
        let timestamp = Date(timeIntervalSinceReferenceDate: 1_000)

        addNotification(
            source: .aiProvider("codex"),
            context: context,
            appState: appState,
            to: store,
            title: "Codex",
            timestamp: timestamp,
            desktopDeliveryIngress: .aiHook(providerID: "codex")
        )
        addNotification(
            source: .osc,
            context: context,
            appState: appState,
            to: store,
            title: "Codex",
            timestamp: timestamp.addingTimeInterval(1),
            desktopDeliveryIngress: .terminalOSC
        )

        #expect(store.notifications.count == 2)
        #expect(desktopNotifier.delivered.count == 1)
    }

    @Test("coalesces default title mismatch for matching AI hook and OSC pair")
    func coalescesDefaultTitleMismatchForMatchingAIHookAndOSCPair() {
        let store = NotificationStore.shared
        store.clear()
        let desktopNotifier = NotificationDesktopNotifierSpy()
        let restoreDesktopDelivery = enableDesktopDelivery(store, notifier: desktopNotifier)
        defer { restoreDesktopDelivery() }
        let context = makeContext()
        let appState = makeAppState()
        let timestamp = Date(timeIntervalSinceReferenceDate: 1_000)

        addNotification(
            source: .aiProvider("codex"),
            context: context,
            appState: appState,
            to: store,
            title: "Task completed!",
            timestamp: timestamp,
            desktopDeliveryIngress: .aiHook(providerID: "codex")
        )
        addNotification(
            source: .osc,
            context: context,
            appState: appState,
            to: store,
            title: "Command executed!",
            timestamp: timestamp.addingTimeInterval(1),
            desktopDeliveryIngress: .terminalOSC
        )

        #expect(store.notifications.count == 2)
        #expect(desktopNotifier.delivered.count == 1)
    }

    @Test("delivers an OSC notification when matching AI hook candidates are ambiguous")
    func deliversOSCNotificationWhenMatchingAIHookCandidatesAreAmbiguous() {
        let store = NotificationStore.shared
        store.clear()
        let desktopNotifier = NotificationDesktopNotifierSpy()
        let restoreDesktopDelivery = enableDesktopDelivery(store, notifier: desktopNotifier)
        defer { restoreDesktopDelivery() }
        let context = makeContext()
        let appState = makeAppState()
        let timestamp = Date(timeIntervalSinceReferenceDate: 1_000)

        addNotification(
            source: .aiProvider("codex"),
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp,
            desktopDeliveryIngress: .aiHook(providerID: "codex")
        )
        addNotification(
            source: .aiProvider("claude"),
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp.addingTimeInterval(0.5),
            desktopDeliveryIngress: .aiHook(providerID: "claude")
        )
        addNotification(
            source: .osc,
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp.addingTimeInterval(1),
            desktopDeliveryIngress: .terminalOSC
        )
        addNotification(
            source: .aiProvider("opencode"),
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp.addingTimeInterval(1.5),
            desktopDeliveryIngress: .aiHook(providerID: "opencode")
        )

        #expect(store.notifications.count == 4)
        #expect(desktopNotifier.delivered.count == 4)
    }

    @Test("consumes the matching pending delivery after desktop suppression")
    func consumesMatchingPendingDeliveryAfterDesktopSuppression() {
        let store = NotificationStore.shared
        store.clear()
        let desktopNotifier = NotificationDesktopNotifierSpy()
        let restoreDesktopDelivery = enableDesktopDelivery(store, notifier: desktopNotifier)
        defer { restoreDesktopDelivery() }
        let context = makeContext()
        let appState = makeAppState()
        let timestamp = Date(timeIntervalSinceReferenceDate: 1_000)

        addNotification(
            source: .aiProvider("codex"),
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp,
            desktopDeliveryIngress: .aiHook(providerID: "codex")
        )
        addNotification(
            source: .osc,
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp.addingTimeInterval(0.5),
            desktopDeliveryIngress: .terminalOSC
        )
        addNotification(
            source: .osc,
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp.addingTimeInterval(1),
            desktopDeliveryIngress: .terminalOSC
        )

        #expect(store.notifications.count == 3)
        #expect(desktopNotifier.delivered.count == 2)
    }

    @Test("does not enroll notifications without a known desktop ingress")
    func doesNotEnrollNotificationsWithoutKnownDesktopIngress() {
        let store = NotificationStore.shared
        store.clear()
        let desktopNotifier = NotificationDesktopNotifierSpy()
        let restoreDesktopDelivery = enableDesktopDelivery(store, notifier: desktopNotifier)
        defer { restoreDesktopDelivery() }
        let context = makeContext()
        let appState = makeAppState()

        addNotification(source: .aiProvider("example.extension.a"), context: context, appState: appState, to: store)
        addNotification(source: .aiProvider("example.extension.b"), context: context, appState: appState, to: store)

        #expect(store.notifications.count == 2)
        #expect(desktopNotifier.delivered.count == 2)
        #expect(store.notifications.allSatisfy { $0.toDTO().source == .aiProvider($0.source.key) })
    }

    @Test("delivers matching AI hook and OSC notifications outside the coalescing window")
    func deliversMatchingAIHookAndOSCNotificationsOutsideTheCoalescingWindow() {
        let store = NotificationStore.shared
        store.clear()
        let desktopNotifier = NotificationDesktopNotifierSpy()
        let restoreDesktopDelivery = enableDesktopDelivery(store, notifier: desktopNotifier)
        defer { restoreDesktopDelivery() }
        let context = makeContext()
        let appState = makeAppState()
        let timestamp = Date(timeIntervalSinceReferenceDate: 1_000)

        addNotification(
            source: .aiProvider("codex"),
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp,
            desktopDeliveryIngress: .aiHook(providerID: "codex")
        )
        addNotification(
            source: .osc,
            context: context,
            appState: appState,
            to: store,
            timestamp: timestamp.addingTimeInterval(2.001),
            desktopDeliveryIngress: .terminalOSC
        )

        #expect(store.notifications.count == 2)
        #expect(desktopNotifier.delivered.count == 2)
    }

    private func addNotification(
        source: MuxyNotification.Source,
        context: NavigationContext,
        appState: AppState,
        to store: NotificationStore,
        title: String = "Codex",
        body: String = "Finished",
        timestamp: Date = Date(),
        desktopDeliveryIngress: NotificationStore.DesktopDeliveryIngress? = nil
    ) {
        store.addWithContext(
            context: context,
            source: source,
            title: title,
            body: body,
            appState: appState,
            timestamp: timestamp,
            desktopDeliveryIngress: desktopDeliveryIngress
        )
    }

    private func enableDesktopDelivery(
        _ store: NotificationStore,
        notifier: NotificationDesktopNotifierSpy
    ) -> () -> Void {
        let defaults = UserDefaults.standard
        let desktopEnabled = defaults.object(forKey: NotificationSettings.Key.desktopEnabled)
        defaults.set(true, forKey: NotificationSettings.Key.desktopEnabled)
        store.desktopNotifier = notifier
        return {
            store.desktopNotifier = nil
            if let desktopEnabled {
                defaults.set(desktopEnabled, forKey: NotificationSettings.Key.desktopEnabled)
            } else {
                defaults.removeObject(forKey: NotificationSettings.Key.desktopEnabled)
            }
            store.clear()
        }
    }

    private func makeContext() -> NavigationContext {
        NavigationContext(
            projectID: UUID(),
            worktreeID: UUID(),
            worktreePath: "/tmp/muxy",
            areaID: UUID(),
            tabID: UUID()
        )
    }

    private func makeAppState() -> AppState {
        AppState(
            selectionStore: NotificationSelectionStoreStub(),
            terminalViews: NotificationTerminalViewRemovingStub(),
            workspacePersistence: NotificationWorkspacePersistenceStub()
        )
    }
}

@MainActor
private final class NotificationSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class NotificationTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}

private final class NotificationWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class NotificationDesktopNotifierSpy: DesktopNotificationDelivering {
    private(set) var delivered: [MuxyNotification] = []

    func deliver(_ notification: MuxyNotification) {
        delivered.append(notification)
    }
}
