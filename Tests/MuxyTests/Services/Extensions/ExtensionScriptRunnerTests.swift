import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionScriptRunner", .serialized)
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
                stores: ExtensionAPIStores()
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
                stores: ExtensionAPIStores()
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
                stores: ExtensionAPIStores()
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
            stores: ExtensionAPIStores()
        )
        ExtensionScriptRunner.shared.evict(extensionID: "test-ext-evict")
        try await ExtensionScriptRunner.shared.runScript(
            extensionID: "test-ext-evict",
            scriptURL: scriptURL,
            appState: appState,
            stores: ExtensionAPIStores()
        )
        ExtensionScriptRunner.shared.evict(extensionID: "test-ext-evict")
    }

    @Test("cancel flag signals registered waiters so blocked threads wake")
    func cancelFlagWakesRegisteredWaiters() async {
        let flag = ScriptCancelFlag()
        let semaphore = DispatchSemaphore(value: 0)
        #expect(flag.register(semaphore))

        let woke = SendableBox(false)
        DispatchQueue.global().async {
            semaphore.wait()
            woke.value = true
        }

        flag.cancel()

        for _ in 0..<50 where !woke.value {
            try? await Task.sleep(for: .milliseconds(10))
        }
        #expect(woke.value)
        #expect(flag.isCancelled)
    }

    @Test("registering on an already cancelled flag is refused")
    func cancelFlagRefusesRegistrationAfterCancel() {
        let flag = ScriptCancelFlag()
        flag.cancel()
        #expect(!flag.register(DispatchSemaphore(value: 0)))
    }

    @Test("dialog cancel is a safe no-op without an active sheet")
    func dialogCancelWithoutActiveSheetIsSafe() {
        ExtensionDialogService.cancel(extensionID: "no-such-ext")
        ExtensionDialogService.cancelAll()
    }

    @Test("modal onSelect keeps the script bridge alive through delivery")
    func modalOnSelectKeepsBridgeAliveThroughDelivery() async throws {
        let extensionID = "test-ext-modal-\(UUID().uuidString)"
        let logDirectory = try makeExtensionDirectory()
        ExtensionLogStore.shared.register(extensionID: extensionID, directory: logDirectory)
        defer {
            ExtensionScriptRunner.shared.evict(extensionID: extensionID)
            ExtensionLogStore.shared.unregister(extensionID: extensionID)
            ExtensionLogStore.shared.flush()
            try? FileManager.default.removeItem(at: logDirectory)
        }

        let appState = makeAppState()
        let scriptURL = try writeScript("""
        muxy.modal.open({
          items: [{ id: 'file', title: 'File' }],
          onSelect(choice) {
            try {
              muxy.tabs.list();
              console.log('modal-dispatch:ok');
            } catch (error) {
              console.log('modal-dispatch:' + error.message);
            }
          },
        });
        """)
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        try await ExtensionScriptRunner.shared.runScript(
            extensionID: extensionID,
            scriptURL: scriptURL,
            appState: appState,
            stores: ExtensionAPIStores()
        )

        ExtensionModalService.shared.select(ExtensionModalService.Item(id: "file", title: "File", subtitle: nil))

        let log = try await waitForLog(extensionID: extensionID, directory: logDirectory, contains: "modal-dispatch:")
        #expect(log.contains("modal-dispatch:permission denied (tabs:read)"))
        #expect(!log.contains("modal-dispatch:bridge released"))
    }

    @Test("modal onQueryChange reaches runScript handlers")
    func modalOnQueryChangeReachesRunScriptHandlers() async throws {
        let extensionID = "test-ext-query-\(UUID().uuidString)"
        let logDirectory = try makeExtensionDirectory()
        ExtensionLogStore.shared.register(extensionID: extensionID, directory: logDirectory)
        defer {
            ExtensionScriptRunner.shared.evict(extensionID: extensionID)
            ExtensionModalService.shared.dismiss()
            ExtensionLogStore.shared.unregister(extensionID: extensionID)
            ExtensionLogStore.shared.flush()
            try? FileManager.default.removeItem(at: logDirectory)
        }

        let appState = makeAppState()
        let scriptURL = try writeScript("""
        muxy.modal.open({
          items: [],
          onQueryChange(query, options) {
            console.log('query-change:' + query + ':' + options.caseSensitive + ':' + options.wholeWord + ':' + options.regex);
            muxy.modal.feed([{ id: 'hit', title: query }]);
            muxy.modal.finish();
          },
        });
        """)
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        try await ExtensionScriptRunner.shared.runScript(
            extensionID: extensionID,
            scriptURL: scriptURL,
            appState: appState,
            stores: ExtensionAPIStores()
        )

        ExtensionModalService.shared.queryChanged(
            "한글",
            options: .init(caseSensitive: true, wholeWord: true, regex: true)
        )

        let log = try await waitForLog(extensionID: extensionID, directory: logDirectory, contains: "query-change:")
        #expect(log.contains("query-change:한글:true:true:true"))
        let page = try await waitForModalPage(query: "한글")
        #expect(page.items.map(\.title) == ["한글"])
    }

    @Test("sync muxy API called from onQuery under rapid queries does not deadlock the main thread")
    func syncApiFromModalQueryDoesNotDeadlock() async throws {
        let extensionID = "test-ext-lockstorm"
        let logDirectory = try makeExtensionDirectory()
        ExtensionLogStore.shared.register(extensionID: extensionID, directory: logDirectory)
        defer {
            ExtensionScriptRunner.shared.evict(extensionID: extensionID)
            ExtensionModalService.shared.dismiss()
            ExtensionLogStore.shared.unregister(extensionID: extensionID)
            ExtensionLogStore.shared.flush()
            try? FileManager.default.removeItem(at: logDirectory)
        }

        let appState = makeAppState()
        let scriptURL = try writeScript("""
        muxy.modal.open({
          items: [],
          onQuery(query, emit) {
            try { muxy.storage.set('last', query); } catch (e) {}
            emit([{ id: query || 'empty', title: query || 'empty' }]);
            return [{ id: query || 'empty', title: query || 'empty' }];
          },
        });
        """)
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        try await ExtensionScriptRunner.shared.runScript(
            extensionID: extensionID,
            scriptURL: scriptURL,
            appState: appState,
            stores: ExtensionAPIStores()
        )

        for round in 0 ..< 40 {
            ExtensionModalService.shared.queryChanged("q-\(round)")
            ExtensionModalService.shared.queryChanged("")
        }

        ExtensionModalService.shared.queryChanged("done")
        let page = try await waitForModalPage(query: "done")
        #expect(page.items.map(\.title).contains("done"))
    }

    @Test("rapid modal queries, clears, feeds, and reruns do not crash or deadlock")
    func rapidModalStormDoesNotCrashOrDeadlock() async throws {
        let extensionID = "test-ext-storm"
        let logDirectory = try makeExtensionDirectory()
        ExtensionLogStore.shared.register(extensionID: extensionID, directory: logDirectory)
        defer {
            ExtensionScriptRunner.shared.evict(extensionID: extensionID)
            ExtensionModalService.shared.dismiss()
            ExtensionLogStore.shared.unregister(extensionID: extensionID)
            ExtensionLogStore.shared.flush()
            try? FileManager.default.removeItem(at: logDirectory)
        }

        let appState = makeAppState()
        let scriptURL = try writeScript("""
        const rows = [];
        for (let i = 0; i < 1000; i++) {
          rows.push({ id: 'row-' + i, title: 'title '.repeat(20) + i, subtitle: 'sub '.repeat(40) + i });
        }
        muxy.modal.open({
          placeholder: 'storm',
          items(emit) {
            for (let b = 0; b < 5; b++) emit(rows.slice(b * 200, (b + 1) * 200));
          },
          onQuery(query, emit) {
            const until = Date.now() + 20;
            while (Date.now() < until) {}
            for (let b = 0; b < 5; b++) emit(rows.slice(b * 200, (b + 1) * 200));
            return rows.slice(0, 200);
          },
          onSelect(choice) {
            console.log('storm-select:' + (choice ? choice.id : 'null'));
          },
        });
        """)
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        let stores = ExtensionAPIStores()
        try await ExtensionScriptRunner.shared.runScript(
            extensionID: extensionID,
            scriptURL: scriptURL,
            appState: appState,
            stores: stores
        )

        for round in 0 ..< 30 {
            ExtensionModalService.shared.queryChanged("query-\(round)")
            try await Task.sleep(for: .milliseconds(8))
            ExtensionModalService.shared.queryChanged("")
            try await Task.sleep(for: .milliseconds(8))

            if round % 5 == 4 {
                try await ExtensionScriptRunner.shared.runScript(
                    extensionID: extensionID,
                    scriptURL: scriptURL,
                    appState: appState,
                    stores: stores
                )
            }
            if round % 7 == 6 {
                ExtensionModalService.shared.select(
                    ExtensionModalService.Item(id: "row-1", title: "t", subtitle: nil)
                )
                try await ExtensionScriptRunner.shared.runScript(
                    extensionID: extensionID,
                    scriptURL: scriptURL,
                    appState: appState,
                    stores: stores
                )
            }
        }

        ExtensionModalService.shared.queryChanged("final")
        let page = try await waitForModalPage(query: "final")
        #expect(!page.items.isEmpty)
    }

    @Test("superseded modal queries are skipped before invoking the handler")
    func supersededModalQueriesAreSkipped() async throws {
        let extensionID = "test-ext-stale-\(UUID().uuidString)"
        let logDirectory = try makeExtensionDirectory()
        ExtensionLogStore.shared.register(extensionID: extensionID, directory: logDirectory)
        defer {
            ExtensionScriptRunner.shared.evict(extensionID: extensionID)
            ExtensionModalService.shared.dismiss()
            ExtensionLogStore.shared.unregister(extensionID: extensionID)
            ExtensionLogStore.shared.flush()
            try? FileManager.default.removeItem(at: logDirectory)
        }

        let appState = makeAppState()
        let scriptURL = try writeScript("""
        muxy.modal.open({
          items: [],
          onQuery(query) {
            console.log('query-start:' + query);
            const until = Date.now() + 300;
            while (Date.now() < until) {}
            console.log('query-end:' + query);
            return [{ id: query, title: query }];
          },
        });
        """)
        defer { try? FileManager.default.removeItem(at: scriptURL.deletingLastPathComponent()) }

        try await ExtensionScriptRunner.shared.runScript(
            extensionID: extensionID,
            scriptURL: scriptURL,
            appState: appState,
            stores: ExtensionAPIStores()
        )

        ExtensionModalService.shared.queryChanged("warm")
        _ = try await waitForLog(extensionID: extensionID, directory: logDirectory, contains: "query-start:warm")
        ExtensionModalService.shared.queryChanged("q1")
        ExtensionModalService.shared.queryChanged("q2")
        ExtensionModalService.shared.queryChanged("q3")

        let log = try await waitForLog(extensionID: extensionID, directory: logDirectory, contains: "query-end:q3")
        #expect(log.contains("query-end:q3"))
        #expect(!log.contains("query-start:q1"))
        #expect(!log.contains("query-start:q2"))
    }

    private func writeScript(_ source: String) throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("script-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let scriptURL = directory.appendingPathComponent("script.js")
        try Data(source.utf8).write(to: scriptURL)
        return scriptURL
    }

    private func makeExtensionDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("script-log-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }

    private func waitForLog(extensionID: String, directory: URL, contains needle: String) async throws -> String {
        let logURL = ExtensionLogStore.shared.logURL(extensionID: extensionID, directory: directory)
        for _ in 0..<50 {
            ExtensionLogStore.shared.flush()
            let text = (try? String(contentsOf: logURL, encoding: .utf8)) ?? ""
            if text.contains(needle) { return text }
            try await Task.sleep(for: .milliseconds(20))
        }
        ExtensionLogStore.shared.flush()
        return (try? String(contentsOf: logURL, encoding: .utf8)) ?? ""
    }

    private func waitForModalPage(query: String) async throws -> ExtensionModalService.Page {
        for _ in 0..<50 {
            if let request = ExtensionModalService.shared.active {
                let page = ExtensionModalService.shared.page(for: request, query: query, offset: 0, limit: 10)
                if !page.items.isEmpty { return page }
            }
            try await Task.sleep(for: .milliseconds(20))
        }
        let request = try #require(ExtensionModalService.shared.active)
        return ExtensionModalService.shared.page(for: request, query: query, offset: 0, limit: 10)
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

private final class SendableBox<T>: @unchecked Sendable {
    private let lock = NSLock()
    private var stored: T

    init(_ value: T) {
        stored = value
    }

    var value: T {
        get {
            lock.lock()
            defer { lock.unlock() }
            return stored
        }
        set {
            lock.lock()
            stored = newValue
            lock.unlock()
        }
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
