import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("ProjectPickerWorkflow")
struct ProjectPickerWorkflowTests {
    @Test("new input applies only latest directory snapshot")
    func latestDirectorySnapshotWins() async {
        let loader = ProjectPickerWorkflowTestDirectoryLoader()
        let workflow = ProjectPickerWorkflow(
            defaultDisplayPath: "~/",
            homeDirectory: "/Users/alice",
            projectPaths: [],
            directoryLoader: { await loader.load($0) },
            reloadDelay: .zero,
            loadingMessageDelay: .seconds(5)
        )

        _ = workflow.setInput("~/First")
        await waitUntil { await loader.hasRequest(for: "~/First") }

        _ = workflow.setInput("~/Second")
        await waitUntil { await loader.hasRequest(for: "~/Second") }

        await loader.resolve(
            input: "~/Second",
            snapshot: ProjectPickerDirectorySnapshot(rows: ["Second"], readFailed: false)
        )
        await waitUntil { workflow.session.rows.map(\.name) == ["Second"] }

        await loader.resolve(
            input: "~/First",
            snapshot: ProjectPickerDirectorySnapshot(rows: ["First"], readFailed: false)
        )
        try? await Task.sleep(for: .milliseconds(20))

        #expect(workflow.session.rows.map(\.name) == ["Second"])
    }

    @Test("loading message appears only while reload is active")
    func loadingMessagePolicy() async {
        let loader = ProjectPickerWorkflowTestDirectoryLoader()
        let slowWorkflow = ProjectPickerWorkflow(
            defaultDisplayPath: "~/Slow",
            homeDirectory: "/Users/alice",
            projectPaths: [],
            directoryLoader: { await loader.load($0) },
            reloadDelay: .zero,
            loadingMessageDelay: .milliseconds(10)
        )

        _ = slowWorkflow.setInput("~/Slow")
        await waitUntil { await loader.hasRequest(for: "~/Slow") }
        await waitUntil { slowWorkflow.session.directoryLoadState.showsMessage }
        #expect(slowWorkflow.session.directoryLoadState == .loading(showsMessage: true))

        let fastWorkflow = ProjectPickerWorkflow(
            defaultDisplayPath: "~/Fast",
            homeDirectory: "/Users/alice",
            projectPaths: [],
            directoryLoader: { _ in ProjectPickerDirectorySnapshot(rows: ["Fast"], readFailed: false) },
            reloadDelay: .zero,
            loadingMessageDelay: .milliseconds(50)
        )

        _ = fastWorkflow.setInput("~/Fast")
        await waitUntil { fastWorkflow.session.directoryLoadState == .loaded }
        try? await Task.sleep(for: .milliseconds(80))

        #expect(fastWorkflow.session.directoryLoadState == .loaded)
    }

    @Test("cancel ignores pending directory snapshot")
    func cancelStopsPendingReloadWork() async {
        let loader = ProjectPickerWorkflowTestDirectoryLoader()
        let workflow = ProjectPickerWorkflow(
            defaultDisplayPath: "~/Canceled",
            homeDirectory: "/Users/alice",
            projectPaths: [],
            directoryLoader: { await loader.load($0) },
            reloadDelay: .zero,
            loadingMessageDelay: .seconds(5)
        )

        _ = workflow.setInput("~/Canceled")
        await waitUntil { await loader.hasRequest(for: "~/Canceled") }
        workflow.cancel()
        await loader.resolve(
            input: "~/Canceled",
            snapshot: ProjectPickerDirectorySnapshot(rows: ["Canceled"], readFailed: false)
        )
        try? await Task.sleep(for: .milliseconds(20))

        #expect(workflow.session.rows.isEmpty)
        #expect(workflow.session.directoryLoadState == .loading(showsMessage: false))
    }

    @Test("folder search applies only the latest query and confirms the selected absolute path")
    func latestFolderSearchWins() async {
        let loader = ProjectPickerWorkflowTestFolderSearchLoader()
        let workflow = ProjectPickerWorkflow(
            defaultDisplayPath: "~/Projects/",
            homeDirectory: "/Users/alice",
            projectPaths: [],
            folderSearchPreparer: { _ in },
            folderSearchLoader: { query, _, _, _ in await loader.load(query) },
            reloadDelay: .zero,
            loadingMessageDelay: .seconds(5)
        )
        let secondResult = ProjectPickerFolderSearchResult(
            name: "muxy",
            path: "/Users/alice/Projects/muxy",
            displayPath: "~/Projects/muxy/"
        )

        _ = workflow.setInput("mu")
        await waitUntil { await loader.hasRequest(for: "mu") }
        _ = workflow.setInput("muxy")
        await waitUntil { await loader.hasRequest(for: "muxy") }

        await loader.resolve(
            query: "muxy",
            snapshot: ProjectPickerFolderSearchSnapshot(results: [secondResult], readFailed: false)
        )
        await waitUntil { workflow.session.searchResults == [secondResult] }

        await loader.resolve(
            query: "mu",
            snapshot: ProjectPickerFolderSearchSnapshot(
                results: [
                    ProjectPickerFolderSearchResult(
                        name: "music",
                        path: "/Users/alice/Music",
                        displayPath: "~/Music/"
                    ),
                ],
                readFailed: false
            )
        )
        try? await Task.sleep(for: .milliseconds(20))

        #expect(workflow.session.searchResults == [secondResult])
        #expect(workflow.handle(.openHighlighted) == [
            .confirmProjectPath(path: secondResult.path, createIfMissing: false),
        ])
        #expect(workflow.handle(.confirmTypedPath) == [
            .confirmProjectPath(path: secondResult.path, createIfMissing: false),
        ])
    }

    @Test("typed path confirmation emits external requests")
    func typedPathConfirmationRequests() throws {
        let existingPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-workflow-existing-\(UUID().uuidString)", isDirectory: true)
            .standardizedFileURL
        try FileManager.default.createDirectory(at: existingPath, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: existingPath) }

        let existingWorkflow = ProjectPickerWorkflow(defaultDisplayPath: existingPath.path, projectPaths: [])
        _ = existingWorkflow.setInput(existingPath.path)
        #expect(existingWorkflow.handle(.confirmTypedPath) == [
            .confirmProjectPath(path: existingPath.path, createIfMissing: false),
        ])

        let missingPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-workflow-\(UUID().uuidString)", isDirectory: true)
            .standardizedFileURL
            .path
        let workflow = ProjectPickerWorkflow(defaultDisplayPath: missingPath, projectPaths: [])
        _ = workflow.setInput(missingPath)

        #expect(workflow.handle(.confirmTypedPath) == [.askCreateDirectory(path: missingPath)])
        #expect(workflow.handleCreateDirectoryDecision(path: missingPath, accepted: false) == [])
        #expect(workflow.handleCreateDirectoryDecision(path: missingPath, accepted: true) == [
            .confirmProjectPath(path: missingPath, createIfMissing: true),
        ])
    }

    @Test("confirmation result requests dismissal or failure presentation")
    func confirmationResultHandling() {
        let workflow = ProjectPickerWorkflow(defaultDisplayPath: "~/", homeDirectory: "/Users/alice", projectPaths: [])

        #expect(workflow.handleProjectPathConfirmationResult(.success, path: "/tmp/muxy") == [.dismiss])
        #expect(workflow.handleProjectPathConfirmationResult(.notDirectory, path: "/tmp/muxy") == [
            .showFailure(ProjectPickerConfirmationFailurePresentation(result: .notDirectory, path: "/tmp/muxy")),
        ])
    }

    @Test("finder and settings actions emit edge requests")
    func edgeSideEffectRequests() {
        let workflow = ProjectPickerWorkflow(defaultDisplayPath: "~/", homeDirectory: "/Users/alice", projectPaths: [])

        #expect(workflow.chooseWithFinder() == [.dismiss, .chooseFinder])
        #expect(workflow.editDefaultLocation() == [.dismiss, .openSettingsFocusedOnDefaultLocation])
    }

    private func waitUntil(
        timeout: Duration = .seconds(1),
        condition: @escaping () async -> Bool
    ) async {
        let start = ContinuousClock.now
        while ContinuousClock.now - start < timeout {
            if await condition() { return }
            try? await Task.sleep(for: .milliseconds(5))
        }
    }
}

private actor ProjectPickerWorkflowTestDirectoryLoader {
    private var requests: Set<String> = []
    private var continuations: [String: CheckedContinuation<ProjectPickerDirectorySnapshot, Never>] = [:]

    func load(_ pathState: ProjectPickerPathState) async -> ProjectPickerDirectorySnapshot {
        requests.insert(pathState.input)
        return await withCheckedContinuation { continuation in
            continuations[pathState.input] = continuation
        }
    }

    func hasRequest(for input: String) -> Bool {
        requests.contains(input)
    }

    func resolve(input: String, snapshot: ProjectPickerDirectorySnapshot) {
        continuations.removeValue(forKey: input)?.resume(returning: snapshot)
    }
}

private actor ProjectPickerWorkflowTestFolderSearchLoader {
    private var requests: Set<String> = []
    private var continuations: [String: CheckedContinuation<ProjectPickerFolderSearchSnapshot, Never>] = [:]

    func load(_ query: String) async -> ProjectPickerFolderSearchSnapshot {
        requests.insert(query)
        return await withCheckedContinuation { continuation in
            continuations[query] = continuation
        }
    }

    func hasRequest(for query: String) -> Bool {
        requests.contains(query)
    }

    func resolve(query: String, snapshot: ProjectPickerFolderSearchSnapshot) {
        continuations.removeValue(forKey: query)?.resume(returning: snapshot)
    }
}
