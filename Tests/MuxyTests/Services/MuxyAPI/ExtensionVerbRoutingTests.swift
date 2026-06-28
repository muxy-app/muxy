import Foundation
import Testing

@testable import Muxy

@Suite("Extension verb routing")
@MainActor
struct ExtensionVerbRoutingTests {
    @Test("MuxyAPI verbNames includes the extension verbs")
    func verbNamesIncludesExtensionVerbs() {
        let verbs = MuxyAPI.Permissions.verbNames
        #expect(verbs.contains("exec"))
        #expect(verbs.contains("extension.settings.get"))
        #expect(verbs.contains("extension.settings.set"))
        #expect(verbs.contains("extension.statusbar.set"))
        #expect(verbs.contains("topbar.set"))
        #expect(verbs.contains("statusbar.set"))
        #expect(verbs.contains("tabs.open"))
    }

    @Test("topbar.set and statusbar.set require panels:write")
    func barItemVerbsRequirePanelsWrite() {
        #expect(MuxyAPI.Permissions.required(for: "topbar.set") == .panelsWrite)
        #expect(MuxyAPI.Permissions.required(for: "statusbar.set") == .panelsWrite)
    }

    @Test("notifications.notify and toast both require notifications:write")
    func notifyVerbRequiresNotificationsWrite() {
        #expect(MuxyAPI.Permissions.required(for: "notifications.notify") == .notificationsWrite)
        #expect(MuxyAPI.Permissions.required(for: "toast") == .notificationsWrite)
    }

    @Test("agent.status and file.changed events require their read permission")
    func gatedEventsRequireReadPermission() {
        #expect(MuxyAPI.Permissions.required(forEvent: ExtensionEventName.agentStatus) == .agentsRead)
        #expect(MuxyAPI.Permissions.required(forEvent: ExtensionEventName.fileChanged) == .filesRead)
    }

    @Test("ungated events require no permission")
    func ungatedEventsRequireNoPermission() {
        #expect(MuxyAPI.Permissions.required(forEvent: ExtensionEventName.paneFocused) == nil)
    }

    @Test("every browser automation verb is registered and permissioned")
    func browserAutomationVerbsAreRegistered() {
        let verbs = [
            "browser.eval", "browser.click", "browser.type", "browser.waitFor",
            "browser.getText", "browser.getHTML", "browser.getAttribute",
            "browser.reload", "browser.back", "browser.forward", "browser.waitForNavigation",
            "browser.screenshot",
            "browser.storage.get", "browser.storage.set", "browser.storage.clear",
            "browser.cookies.get", "browser.cookies.set", "browser.cookies.delete", "browser.cookies.clear",
            "browser.wait", "browser.fill", "browser.press", "browser.select", "browser.hover",
            "browser.scrollIntoView", "browser.setChecked", "browser.is",
            "browser.getValue", "browser.getCount", "browser.find", "browser.snapshot",
        ]
        for verb in verbs {
            #expect(MuxyAPI.Permissions.verbNames.contains(verb), "verbNames missing \(verb)")
            #expect(MuxyAPI.Permissions.required(for: verb) != nil, "no permission mapped for \(verb)")
        }
    }

    @Test("browser write verbs require browser:write and read verbs require browser:read")
    func browserAutomationPermissionsAreCorrect() {
        let writeVerbs = [
            "browser.eval", "browser.click", "browser.type", "browser.reload",
            "browser.back", "browser.forward",
            "browser.storage.set", "browser.storage.clear",
            "browser.cookies.set", "browser.cookies.delete", "browser.cookies.clear",
            "browser.fill", "browser.press", "browser.select", "browser.hover",
            "browser.scrollIntoView", "browser.setChecked",
        ]
        let readVerbs = [
            "browser.waitFor", "browser.getText", "browser.getHTML", "browser.getAttribute",
            "browser.waitForNavigation", "browser.screenshot",
            "browser.storage.get", "browser.cookies.get",
            "browser.wait", "browser.is", "browser.getValue", "browser.getCount",
            "browser.find", "browser.snapshot",
        ]
        for verb in writeVerbs {
            #expect(MuxyAPI.Permissions.required(for: verb) == .browserWrite, "\(verb) should be browser:write")
        }
        for verb in readVerbs {
            #expect(MuxyAPI.Permissions.required(for: verb) == .browserRead, "\(verb) should be browser:read")
        }
    }

    @Test("browser.wait escalates to browser:write when given a function condition")
    func browserWaitFunctionRequiresWrite() {
        #expect(MuxyAPI.Permissions.required(for: "browser.wait", args: [:]) == .browserRead)
        #expect(MuxyAPI.Permissions.required(for: "browser.wait", args: ["selector": "a"]) == .browserRead)
        #expect(MuxyAPI.Permissions.required(for: "browser.wait", args: ["function": ""]) == .browserRead)
        #expect(MuxyAPI.Permissions.required(for: "browser.wait", args: ["function": "1+1"]) == .browserWrite)
    }

    @Test("MuxyAPI verbNames includes the legacy CLI verbs")
    func verbNamesIncludesLegacyVerbs() {
        let verbs = MuxyAPI.Permissions.verbNames
        for verb in ["split-right", "split-down", "send", "send-keys", "read-screen", "open-tab", "list-tabs"] {
            #expect(verbs.contains(verb), "verbNames missing legacy verb \(verb)")
        }
    }

    @Test("extension.settings.get without identify returns error")
    func settingsGetRequiresIdentify() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest(
            "extension.settings.get|missing",
            appState: appState
        )
        #expect(result == "error:identify required")
    }

    @Test("extension.settings.set without identify returns error")
    func settingsSetRequiresIdentify() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest(
            "extension.settings.set|key|true",
            appState: appState
        )
        #expect(result == "error:identify required")
    }

    @Test("extension.statusbar.set without identify returns error")
    func statusBarSetRequiresIdentify() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest(
            "extension.statusbar.set|item|text",
            appState: appState
        )
        #expect(result == "error:identify required")
    }

    @Test("extension.settings.set rejects oversize payload")
    func settingsSetRejectsOversize() async {
        let appState = makeAppState()
        let big = String(repeating: "a", count: 65 * 1024)
        let payload = "\"\(big)\""
        let result = await SocketCommandHandler.handleRequest(
            "extension.settings.set|k|\(payload)",
            appState: appState,
            clientContext: .init(extensionID: "ghost")
        )
        #expect(result.hasPrefix("error:value exceeds"))
    }

    @Test("extension.settings.set rejects unknown extension")
    func settingsSetUnknownExtension() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest(
            "extension.settings.set|key|true",
            appState: appState,
            clientContext: .init(extensionID: "ghost-extension-xyz")
        )
        #expect(result == "error:unknown extension")
    }

    @Test("extension.statusbar.set rejects unknown item")
    func statusBarSetUnknownItem() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest(
            "extension.statusbar.set|nope|hello",
            appState: appState,
            clientContext: .init(extensionID: "ghost-extension-xyz")
        )
        #expect(result.hasPrefix("error:"))
    }

    @Test("topbar.set and statusbar.set without identify return error")
    func barItemSetRequiresIdentify() async {
        let appState = makeAppState()
        for verb in ["topbar.set", "statusbar.set"] {
            let result = await SocketCommandHandler.handleRequest("\(verb)|e30=", appState: appState)
            #expect(result == "error:identify required")
        }
    }

    @Test("dynamic modal routes query requests with options")
    func dynamicModalRoutesQueryRequestsWithOptions() async throws {
        ExtensionModalService.shared.dismiss()
        defer { ExtensionModalService.shared.dismiss() }
        let appState = makeAppState()
        var receivedQueryID = 0
        var receivedQuery = ""
        var receivedOptions = ExtensionModalSearchOptions()
        let result = try await MuxyAPIDispatcher.dispatch(
            verb: "modal.open",
            args: ["dynamic": true],
            context: MuxyAPIDispatcher.Context(
                extensionID: "demo",
                appState: appState,
                projectStore: nil,
                worktreeStore: nil,
                projectGroupStore: nil
            )
        ) as? [String: String]
        let requestID = try #require(result?["requestID"])

        ExtensionModalService.shared.onQueryRequest(requestID: requestID) { queryID, query, options in
                    receivedQueryID = queryID
                    receivedQuery = query
                    receivedOptions = options
        }

        ExtensionModalService.shared.requestQuery(query: "abc", options: .init(caseSensitive: true, wholeWord: true))

        #expect(receivedQueryID == 1)
        #expect(receivedQuery == "abc")
        #expect(receivedOptions == .init(caseSensitive: true, wholeWord: true))
    }

    @Test("extension.statusbar.set with empty text is treated as clear")
    func statusBarSetEmptyClears() async {
        let appState = makeAppState()
        let resultExplicit = await SocketCommandHandler.handleRequest(
            "extension.statusbar.set|item|",
            appState: appState,
            clientContext: .init(extensionID: "ghost-extension-xyz")
        )
        let resultImplicit = await SocketCommandHandler.handleRequest(
            "extension.statusbar.set|item",
            appState: appState,
            clientContext: .init(extensionID: "ghost-extension-xyz")
        )
        #expect(resultExplicit == resultImplicit)
    }

    private func makeAppState() -> AppState {
        AppState(
            selectionStore: SelectionStoreNoop(),
            terminalViews: TerminalViewNoop(),
            workspacePersistence: WorkspacePersistenceNoop()
        )
    }
}

private final class WorkspacePersistenceNoop: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class SelectionStoreNoop: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class TerminalViewNoop: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
