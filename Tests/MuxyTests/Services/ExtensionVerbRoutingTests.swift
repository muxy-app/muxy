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
