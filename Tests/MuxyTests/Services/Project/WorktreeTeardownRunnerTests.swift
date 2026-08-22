import Foundation
import Testing

@testable import Muxy

@Suite("WorktreeTeardownRunner")
struct WorktreeTeardownRunnerTests {
    @Test("WorktreeConfig decodes teardown strings and objects")
    func configDecodesTeardownCommands() throws {
        let json = """
        {
          "setup": ["pnpm install"],
          "teardown": [
            "docker compose down",
            { "name": "cleanup", "command": "rm -rf tmp" }
          ]
        }
        """

        let config = try JSONDecoder().decode(WorktreeConfig.self, from: Data(json.utf8))

        #expect(config.setup.map(\.command) == ["pnpm install"])
        #expect(config.teardown.map(\.command) == ["docker compose down", "rm -rf tmp"])
        #expect(config.teardown[1].name == "cleanup")
    }

    @Test("global config path uses XDG_CONFIG_HOME when available")
    func globalConfigPathUsesXDGConfigHome() {
        let home = URL(fileURLWithPath: "/Users/example", isDirectory: true)

        let xdgURL = WorktreeConfig.globalConfigURL(
            homeDirectory: home,
            environment: ["XDG_CONFIG_HOME": "/tmp/config"]
        )
        let fallbackURL = WorktreeConfig.globalConfigURL(homeDirectory: home, environment: [:])

        #expect(xdgURL.path == "/tmp/config/muxy/worktree.json")
        #expect(fallbackURL.path == "/Users/example/.config/muxy/worktree.json")
    }

    @Test("global and project commands compose in lifecycle order")
    func commandsComposeInLifecycleOrder() throws {
        let projectPath = try makeProjectConfig(setup: ["project up"], teardown: ["project down"])
        let globalConfigURL = try makeGlobalConfig(setup: ["global up"], teardown: ["global down"])

        let setup = try WorktreeConfig.setupCommands(
            sourceProjectPath: projectPath,
            globalConfigURL: globalConfigURL
        )
        let resolvedSetup = try WorktreeConfig.resolvedSetupCommands(
            sourceProjectPath: projectPath,
            globalConfigURL: globalConfigURL
        )
        let teardown = try WorktreeConfig.teardownCommands(
            sourceProjectPath: projectPath,
            globalConfigURL: globalConfigURL
        )
        let resolvedTeardown = try WorktreeConfig.resolvedTeardownCommands(
            sourceProjectPath: projectPath,
            globalConfigURL: globalConfigURL
        )

        #expect(setup.map(\.command) == ["global up", "project up"])
        #expect(resolvedSetup.map(\.source) == [.global, .project])
        #expect(teardown.map(\.command) == ["project down", "global down"])
        #expect(resolvedTeardown.map(\.source) == [.project, .global])
    }

    @Test("setup runner executes global then project commands with worktree environment")
    func setupRunnerExecutesLayeredCommands() async throws {
        let projectPath = try makeProjectConfig(setup: ["project follows"], teardown: [])
        let globalConfigURL = try makeGlobalConfig(setup: ["global first"], teardown: [])
        let approval = WorktreeConfig.ProjectHookApproval(resolvedCommands: try WorktreeConfig.resolvedSetupCommands(
            sourceProjectPath: projectPath,
            globalConfigURL: globalConfigURL
        ))
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: "feature/test",
            source: .muxy,
            isPrimary: false
        )
        let capture = SetupExecutionCapture()

        await WorktreeSetupRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            projectHookApproval: approval,
            globalConfigURL: globalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands == ["global first", "project follows"])
        #expect(capture.environments.allSatisfy { $0["MUXY_PROJECT_PATH"] == projectPath })
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_PATH"] == worktree.path })
    }

    @Test("setup runner skips project commands without approval")
    func setupRunnerSkipsUnapprovedProjectCommands() async throws {
        let projectPath = try makeProjectConfig(setup: ["project setup"], teardown: [])
        let globalConfigURL = try makeGlobalConfig(setup: ["global setup"], teardown: [])
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: "feature/test",
            source: .muxy,
            isPrimary: false
        )
        let capture = SetupExecutionCapture()

        await WorktreeSetupRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            globalConfigURL: globalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands == ["global setup"])
    }

    @Test("run executes global teardown without a project config")
    func runExecutesGlobalTeardownWithoutProjectConfig() async throws {
        let projectPath = try makeDirectory(prefix: "muxy-project")
        let globalConfigURL = try makeGlobalConfig(teardown: ["global cleanup"])
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: "feature/test",
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        try await WorktreeTeardownRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            globalConfigURL: globalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands == ["global cleanup"])
        #expect(capture.environments.first?["MUXY_PROJECT_PATH"] == projectPath)
    }

    @Test("invalid global teardown config blocks command execution")
    func invalidGlobalTeardownConfigBlocksExecution() async throws {
        let projectPath = try makeDirectory(prefix: "muxy-project")
        let globalConfigURL = try makeInvalidGlobalConfig()
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: nil,
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        await #expect(throws: WorktreeConfigError.self) {
            try await WorktreeTeardownRunner.run(
                sourceProjectPath: projectPath,
                worktree: worktree,
                globalConfigURL: globalConfigURL,
                executor: capture.executor(returning: 0)
            )
        }
        #expect(capture.commands.isEmpty)
    }

    @Test("invalid project setup field blocks command resolution")
    func invalidProjectSetupFieldBlocksResolution() throws {
        let projectPath = try makeInvalidProjectConfig(contents: #"{"setup":true}"#)

        #expect(throws: WorktreeConfigError.self) {
            try WorktreeConfig.setupCommands(
                sourceProjectPath: projectPath,
                globalConfigURL: missingGlobalConfigURL
            )
        }
    }

    @Test("unapproved invalid project teardown does not block per-machine commands")
    func unapprovedInvalidProjectTeardownDoesNotBlockGlobalCommands() async throws {
        let projectPath = try makeInvalidProjectConfig(contents: #"{"teardown":true}"#)
        let globalConfigURL = try makeGlobalConfig(teardown: ["global cleanup"])
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: nil,
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        try await WorktreeTeardownRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            globalConfigURL: globalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands == ["global cleanup"])
    }

    @Test("run executes teardown commands with worktree environment")
    func runExecutesTeardownCommandsWithEnvironment() async throws {
        let projectPath = try makeProjectConfig(teardown: [" first ", "", "second"])
        let approval = try projectTeardownApproval(projectPath: projectPath)
        let worktreePath = try makeWorktreeDirectory()
        let worktree = Worktree(
            name: "Feature",
            path: worktreePath,
            branch: "feature/test",
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        try await WorktreeTeardownRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            projectHookApproval: approval,
            globalConfigURL: missingGlobalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands == ["first", "second"])
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_ID"] == worktree.id.uuidString })
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_PATH"] == worktreePath })
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_NAME"] == "Feature" })
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_BRANCH"] == "feature/test" })
    }

    @Test("run skips project teardown without approval")
    func runSkipsUnapprovedProjectTeardown() async throws {
        let projectPath = try makeProjectConfig(teardown: ["project cleanup"])
        let globalConfigURL = try makeGlobalConfig(teardown: ["global cleanup"])
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: nil,
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        try await WorktreeTeardownRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            globalConfigURL: globalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands == ["global cleanup"])
    }

    @Test("run rejects project teardown changed after approval")
    func runRejectsChangedProjectTeardown() async throws {
        let approvedProjectPath = try makeProjectConfig(teardown: ["approved cleanup"])
        let approval = try projectTeardownApproval(projectPath: approvedProjectPath)
        let changedProjectPath = try makeProjectConfig(teardown: ["changed cleanup"])
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: nil,
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        await #expect(throws: WorktreeConfigError.self) {
            try await WorktreeTeardownRunner.run(
                sourceProjectPath: changedProjectPath,
                worktree: worktree,
                projectHookApproval: approval,
                globalConfigURL: missingGlobalConfigURL,
                executor: capture.executor(returning: 0)
            )
        }
        #expect(capture.commands.isEmpty)
    }

    @Test("run skips teardown when the worktree folder is gone")
    func runSkipsWhenWorktreeFolderMissing() async throws {
        let projectPath = try makeProjectConfig(teardown: ["cleanup"])
        let worktree = Worktree(
            name: "Feature",
            path: "/tmp/muxy-missing-\(UUID().uuidString)",
            branch: nil,
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        try await WorktreeTeardownRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            globalConfigURL: missingGlobalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands.isEmpty)
    }

    @Test("run skips externally managed worktrees")
    func runSkipsExternalWorktrees() async throws {
        let projectPath = try makeProjectConfig(teardown: ["cleanup"])
        let worktree = Worktree(
            name: "External",
            path: "/tmp/external",
            branch: "external",
            source: .external,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        try await WorktreeTeardownRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            globalConfigURL: missingGlobalConfigURL,
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands.isEmpty)
    }

    @Test("run stops and throws on teardown failure")
    func runStopsOnFailure() async throws {
        let projectPath = try makeProjectConfig(teardown: ["fail", "after"])
        let approval = try projectTeardownApproval(projectPath: projectPath)
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: nil,
            source: .muxy,
            isPrimary: false
        )
        let capture = ExecutionCapture()

        await #expect(throws: WorktreeTeardownError.self) {
            try await WorktreeTeardownRunner.run(
                sourceProjectPath: projectPath,
                worktree: worktree,
                projectHookApproval: approval,
                globalConfigURL: missingGlobalConfigURL,
                executor: capture.executor(returning: 1)
            )
        }
        #expect(capture.commands == ["fail"])
    }

    @Test("run streams command and output lines to the emit closure")
    func runStreamsOutputLines() async throws {
        let projectPath = try makeProjectConfig(teardown: ["echo hello"])
        let approval = try projectTeardownApproval(projectPath: projectPath)
        let worktree = Worktree(
            name: "Feature",
            path: try makeWorktreeDirectory(),
            branch: nil,
            source: .muxy,
            isPrimary: false
        )
        let collected = LineCollector()

        try await WorktreeTeardownRunner.run(
            sourceProjectPath: projectPath,
            worktree: worktree,
            projectHookApproval: approval,
            emit: { collected.append($0) },
            globalConfigURL: missingGlobalConfigURL,
            executor: { _, _, _, _, emit in
                emit(WorktreeTeardownOutputLine(channel: .stdout, text: "hello"))
                return 0
            }
        )

        let lines = collected.snapshot()
        #expect(lines.contains(where: { $0.channel == .command && $0.text == "$ echo hello" }))
        #expect(lines.contains(where: { $0.channel == .stdout && $0.text == "hello" }))
    }

    @Test("process captures final stdout and stderr without trailing newlines")
    func processCapturesFinalOutputWithoutTrailingNewlines() async throws {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-teardown-process-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let collected = LineCollector()

        let status = try await WorktreeTeardownProcess.run(
            command: "printf out; printf err >&2",
            workingDirectory: directory.path,
            environment: ProcessInfo.processInfo.environment,
            emit: { collected.append($0) }
        )

        let lines = collected.snapshot()
        #expect(status == 0)
        #expect(lines.contains(where: { $0.channel == .stdout && $0.text == "out" }))
        #expect(lines.contains(where: { $0.channel == .stderr && $0.text == "err" }))
    }

    @Test("process terminates when its timeout expires")
    func processTimesOut() async {
        let directory = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-teardown-timeout-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let collected = LineCollector()

        await #expect(throws: SubprocessRunnerError.self) {
            try await WorktreeTeardownProcess.run(
                command: "printf partial; sleep 10 &",
                workingDirectory: directory.path,
                environment: ProcessInfo.processInfo.environment,
                timeout: 0.5,
                emit: { collected.append($0) }
            )
        }
        #expect(collected.snapshot().contains { $0.channel == .stdout && $0.text == "partial" })
    }

    private func makeWorktreeDirectory() throws -> String {
        try makeDirectory(prefix: "muxy-teardown-worktree")
    }

    private func projectTeardownApproval(projectPath: String) throws -> WorktreeConfig.ProjectHookApproval {
        WorktreeConfig.ProjectHookApproval(resolvedCommands: try WorktreeConfig.resolvedTeardownCommands(
            sourceProjectPath: projectPath,
            globalConfigURL: missingGlobalConfigURL
        ))
    }

    private func makeProjectConfig(setup: [String] = [], teardown: [String]) throws -> String {
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-teardown-tests-\(UUID().uuidString)", isDirectory: true)
        let configURL = root
            .appendingPathComponent(".muxy", isDirectory: true)
            .appendingPathComponent("worktree.json")
        try writeConfig(setup: setup, teardown: teardown, to: configURL)
        return root.path
    }

    private func makeGlobalConfig(setup: [String] = [], teardown: [String]) throws -> URL {
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-global-config-tests-\(UUID().uuidString)", isDirectory: true)
        let configURL = root.appendingPathComponent("worktree.json")
        try writeConfig(setup: setup, teardown: teardown, to: configURL)
        return configURL
    }

    private func makeInvalidGlobalConfig() throws -> URL {
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-invalid-global-config-tests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let configURL = root.appendingPathComponent("worktree.json")
        try #"{"teardown":true}"#.write(to: configURL, atomically: true, encoding: .utf8)
        return configURL
    }

    private func makeInvalidProjectConfig(contents: String) throws -> String {
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-invalid-project-config-tests-\(UUID().uuidString)", isDirectory: true)
        let configURL = root
            .appendingPathComponent(".muxy", isDirectory: true)
            .appendingPathComponent("worktree.json")
        try FileManager.default.createDirectory(
            at: configURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try contents.write(to: configURL, atomically: true, encoding: .utf8)
        return root.path
    }

    private func writeConfig(setup: [String], teardown: [String], to url: URL) throws {
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let data = try JSONEncoder().encode(WorktreeConfig(
            setup: setup.map { WorktreeConfig.SetupCommand(command: $0) },
            teardown: teardown.map { WorktreeConfig.SetupCommand(command: $0) }
        ))
        try data.write(to: url)
    }

    private func makeDirectory(prefix: String) throws -> String {
        let url = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("\(prefix)-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.path
    }

    private var missingGlobalConfigURL: URL {
        URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-missing-global-config-\(UUID().uuidString)")
    }
}

private final class ExecutionCapture: @unchecked Sendable {
    private let queue = DispatchQueue(label: "tests.execution-capture")
    private var _commands: [String] = []
    private var _environments: [[String: String]] = []

    var commands: [String] { queue.sync { _commands } }
    var environments: [[String: String]] { queue.sync { _environments } }

    func executor(returning status: Int32) -> WorktreeTeardownRunner.Executor {
        { command, _, environment, _, _ in
            self.queue.sync {
                self._commands.append(command)
                self._environments.append(environment)
            }
            return status
        }
    }
}

private final class SetupExecutionCapture: @unchecked Sendable {
    private let queue = DispatchQueue(label: "tests.setup-execution-capture")
    private var _commands: [String] = []
    private var _environments: [[String: String]] = []

    var commands: [String] { queue.sync { _commands } }
    var environments: [[String: String]] { queue.sync { _environments } }

    func executor(returning status: Int32) -> WorktreeSetupRunner.Executor {
        { command, _, environment, _ in
            self.queue.sync {
                self._commands.append(command)
                self._environments.append(environment)
            }
            return status
        }
    }
}

private final class LineCollector: @unchecked Sendable {
    private let queue = DispatchQueue(label: "tests.line-collector")
    private var lines: [WorktreeTeardownOutputLine] = []

    func append(_ line: WorktreeTeardownOutputLine) {
        queue.sync { lines.append(line) }
    }

    func snapshot() -> [WorktreeTeardownOutputLine] {
        queue.sync { lines }
    }
}
