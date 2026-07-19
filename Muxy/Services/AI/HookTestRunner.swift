import Foundation

enum HookTestResult: Equatable {
    case passed
    case failed(String)
}

struct HookTestRunner {
    struct ProcessOutcome: Equatable {
        let terminationStatus: Int32
        let standardError: String
    }

    typealias Runner = @Sendable (
        _ binaryPath: String,
        _ arguments: [String],
        _ environment: [String: String],
        _ timeout: TimeInterval
    ) throws -> ProcessOutcome

    static let defaultTimeout: TimeInterval = 5

    private let binaryPath: String
    private let socketPath: String
    private let fileExists: @Sendable (String) -> Bool
    private let timeout: TimeInterval
    private let runner: Runner

    init(
        binaryPath: String = MuxyNotificationHooks.hookBinaryPath,
        socketPath: String = NotificationSocketServer.socketPath,
        fileExists: @escaping @Sendable (String) -> Bool = { FileManager.default.fileExists(atPath: $0) },
        timeout: TimeInterval = HookTestRunner.defaultTimeout,
        runner: @escaping Runner = HookTestRunner.runProcess
    ) {
        self.binaryPath = binaryPath
        self.socketPath = socketPath
        self.fileExists = fileExists
        self.timeout = timeout
        self.runner = runner
    }

    static func arguments(providerSocketType: String, providerTitle: String) -> [String] {
        [
            "agent-event",
            "--provider", providerSocketType,
            "--provider-title", providerTitle,
            "--event", "test",
            "--test",
        ]
    }

    func run(providerSocketType: String, providerTitle: String) -> HookTestResult {
        guard fileExists(binaryPath) else {
            return .failed("Hook binary is not staged")
        }
        let environment = ["MUXY_SOCKET_PATH": socketPath]
        do {
            let outcome = try runner(
                binaryPath,
                Self.arguments(providerSocketType: providerSocketType, providerTitle: providerTitle),
                environment,
                timeout
            )
            return Self.interpret(outcome)
        } catch {
            return .failed(error.localizedDescription)
        }
    }

    static func interpret(_ outcome: ProcessOutcome) -> HookTestResult {
        guard outcome.terminationStatus == 0 else {
            let detail = outcome.standardError.trimmingCharacters(in: .whitespacesAndNewlines)
            return .failed(detail.isEmpty ? "Hook exited with status \(outcome.terminationStatus)" : detail)
        }
        return .passed
    }

    static func runProcess(
        binaryPath: String,
        arguments: [String],
        environment: [String: String],
        timeout: TimeInterval
    ) throws -> ProcessOutcome {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: binaryPath)
        process.arguments = arguments
        process.environment = ProcessInfo.processInfo.environment.merging(environment) { _, new in new }

        let errorPipe = Pipe()
        process.standardError = errorPipe
        let inputPipe = Pipe()
        process.standardInput = inputPipe

        let collector = StandardErrorCollector()
        let readHandle = errorPipe.fileHandleForReading
        readHandle.readabilityHandler = { handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                handle.readabilityHandler = nil
                collector.finish()
                return
            }
            collector.append(data)
        }

        let exited = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in exited.signal() }

        try process.run()
        try? inputPipe.fileHandleForWriting.close()

        let timedOut = exited.wait(timeout: .now() + timeout) == .timedOut
        if timedOut {
            terminate(process)
        }

        process.waitUntilExit()
        collector.waitForCompletion(timeout: Self.drainTimeout)
        readHandle.readabilityHandler = nil
        try? readHandle.close()

        if timedOut {
            return ProcessOutcome(terminationStatus: -1, standardError: "Hook timed out")
        }
        return ProcessOutcome(
            terminationStatus: process.terminationStatus,
            standardError: collector.string()
        )
    }

    static let drainTimeout: TimeInterval = 1
    static let terminationGrace: TimeInterval = 0.5

    private static func terminate(_ process: Process) {
        process.terminate()
        let escalation = Date().addingTimeInterval(terminationGrace)
        while process.isRunning, Date() < escalation {
            usleep(10000)
        }
        guard process.isRunning else { return }
        kill(process.processIdentifier, SIGKILL)
    }

    private final class StandardErrorCollector: @unchecked Sendable {
        private let lock = NSLock()
        private let completed = DispatchSemaphore(value: 0)
        private var data = Data()
        private var isFinished = false

        func append(_ chunk: Data) {
            lock.lock()
            data.append(chunk)
            lock.unlock()
        }

        func finish() {
            lock.lock()
            let alreadyFinished = isFinished
            isFinished = true
            lock.unlock()
            guard !alreadyFinished else { return }
            completed.signal()
        }

        func waitForCompletion(timeout: TimeInterval) {
            _ = completed.wait(timeout: .now() + timeout)
        }

        func string() -> String {
            lock.lock()
            defer { lock.unlock() }
            return String(data: data, encoding: .utf8) ?? ""
        }
    }
}
