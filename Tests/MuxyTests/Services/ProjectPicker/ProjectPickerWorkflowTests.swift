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

    @Test("typed path confirmation emits external requests")
    func typedPathConfirmationRequests() throws {
        let existingPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-workflow-existing-\(UUID().uuidString)", isDirectory: true)
            .standardizedFileURL
        try FileManager.default.createDirectory(at: existingPath, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: existingPath) }

        let existingWorkflow = ProjectPickerWorkflow(defaultDisplayPath: existingPath.path, projectPaths: [])
        #expect(existingWorkflow.handle(.confirmTypedPath) == [
            .confirmProjectPath(path: existingPath.path, createIfMissing: false),
        ])

        let missingPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-project-picker-workflow-\(UUID().uuidString)", isDirectory: true)
            .standardizedFileURL
            .path
        let workflow = ProjectPickerWorkflow(defaultDisplayPath: missingPath, projectPaths: [])

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
