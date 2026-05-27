import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionScriptRunner")
@MainActor
struct ExtensionScriptRunnerTests {
    private let testPath = "/tmp/test"

    @Test("script without permission gets denied")
    func scriptWithoutPermissionFails() async throws {
        let appState = makeAppState()
        let scriptURL = try writeScript("muxy.tabs.list();")
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        do {
            try await ExtensionScriptRunner.shared.runScript(
                extensionID: "test-ext-deny",
                scriptURL: scriptURL,
                appState: appState,
                projectStore: nil,
                worktreeStore: nil
            )
            Issue.record("expected throw")
        } catch let error as ExtensionScriptRunner.RunError {
            switch error {
            case let .evaluationFailed(message):
                #expect(message.contains("permission denied"))
            default:
                Issue.record("expected evaluationFailed, got \(error)")
            }
        }
        ExtensionScriptRunner.shared.evict(extensionID: "test-ext-deny")
    }

    @Test("script that throws surfaces as RunError.evaluationFailed")
    func scriptThrowsSurfacesError() async throws {
        let appState = makeAppState()
        let scriptURL = try writeScript("throw new Error('boom');")
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        do {
            try await ExtensionScriptRunner.shared.runScript(
                extensionID: "test-ext-throw",
                scriptURL: scriptURL,
                appState: appState,
                projectStore: nil,
                worktreeStore: nil
            )
            Issue.record("expected throw")
        } catch let error as ExtensionScriptRunner.RunError {
            switch error {
            case let .evaluationFailed(message):
                #expect(message.contains("boom"))
            default:
                Issue.record("expected evaluationFailed, got \(error)")
            }
        }
        ExtensionScriptRunner.shared.evict(extensionID: "test-ext-throw")
    }

    @Test("missing script file fails with scriptUnreadable")
    func missingScriptFails() async throws {
        let appState = makeAppState()
        let missing = FileManager.default.temporaryDirectory
            .appendingPathComponent("does-not-exist-\(UUID().uuidString).js")

        do {
            try await ExtensionScriptRunner.shared.runScript(
                extensionID: "test-ext-missing",
                scriptURL: missing,
                appState: appState,
                projectStore: nil,
                worktreeStore: nil
            )
            Issue.record("expected throw")
        } catch let error as ExtensionScriptRunner.RunError {
            switch error {
            case .scriptUnreadable: break
            default: Issue.record("expected scriptUnreadable, got \(error)")
            }
        }
    }

    @Test("evict drops cached context")
    func evictDropsCache() async throws {
        let appState = makeAppState()
        let scriptURL = try writeScript("globalThis.__counter = (globalThis.__counter || 0) + 1;")
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        try await ExtensionScriptRunner.shared.runScript(
            extensionID: "test-ext-evict",
            scriptURL: scriptURL,
            appState: appState,
            projectStore: nil,
            worktreeStore: nil
        )
        ExtensionScriptRunner.shared.evict(extensionID: "test-ext-evict")
        try await ExtensionScriptRunner.shared.runScript(
            extensionID: "test-ext-evict",
            scriptURL: scriptURL,
            appState: appState,
            projectStore: nil,
            worktreeStore: nil
        )
        ExtensionScriptRunner.shared.evict(extensionID: "test-ext-evict")
    }

    private func writeScript(_ source: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("script-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let scriptURL = directory.appendingPathComponent("script.js")
        try Data(source.utf8).write(to: scriptURL)
        return scriptURL
    }

    private func makeAppState(
        projectID: UUID = UUID(),
        worktreeID: UUID = UUID()
    ) -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
