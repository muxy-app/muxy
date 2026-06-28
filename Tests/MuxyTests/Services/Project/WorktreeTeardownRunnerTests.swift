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

    @Test("run executes teardown commands with worktree environment")
    func runExecutesTeardownCommandsWithEnvironment() async throws {
        let projectPath = try makeProjectConfig(teardown: [" first ", "", "second"])
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
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands == ["first", "second"])
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_PATH"] == worktreePath })
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_NAME"] == "Feature" })
        #expect(capture.environments.allSatisfy { $0["MUXY_WORKTREE_BRANCH"] == "feature/test" })
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
            executor: capture.executor(returning: 0)
        )

        #expect(capture.commands.isEmpty)
    }

    @Test("run stops and throws on teardown failure")
    func runStopsOnFailure() async throws {
        let projectPath = try makeProjectConfig(teardown: ["fail", "after"])
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
                executor: capture.executor(returning: 1)
            )
        }
        #expect(capture.commands == ["fail"])
    }

    @Test("run streams command and output lines to the emit closure")
    func runStreamsOutputLines() async throws {
        let projectPath = try makeProjectConfig(teardown: ["echo hello"])
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
            emit: { collected.append($0) },
            executor: { _, _, _, emit in
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

    private func makeWorktreeDirectory() throws -> String {
        let path = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-teardown-worktree-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: path, withIntermediateDirectories: true)
        return path.path
    }

    private func makeProjectConfig(teardown: [String]) throws -> String {
        let root = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent("muxy-teardown-tests-\(UUID().uuidString)", isDirectory: true)
        let configDirectory = root.appendingPathComponent(".muxy", isDirectory: true)
        try FileManager.default.createDirectory(at: configDirectory, withIntermediateDirectories: true)
        let data = try JSONEncoder().encode(WorktreeConfig(
            setup: [],
            teardown: teardown.map { WorktreeConfig.SetupCommand(command: $0) }
        ))
        try data.write(to: configDirectory.appendingPathComponent("worktree.json"))
        return root.path
    }
}

private final class ExecutionCapture: @unchecked Sendable {
    private let queue = DispatchQueue(label: "tests.execution-capture")
    private var _commands: [String] = []
    private var _environments: [[String: String]] = []

    var commands: [String] { queue.sync { _commands } }
    var environments: [[String: String]] { queue.sync { _environments } }

    func executor(returning status: Int32) -> WorktreeTeardownRunner.Executor {
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
