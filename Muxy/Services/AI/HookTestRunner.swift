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

        try process.run()
        try? inputPipe.fileHandleForWriting.close()

        let deadline = Date().addingTimeInterval(timeout)
        while process.isRunning, Date() < deadline {
            usleep(20000)
        }
        if process.isRunning {
            process.terminate()
            return ProcessOutcome(terminationStatus: -1, standardError: "Hook timed out")
        }

        let errorData = errorPipe.fileHandleForReading.readDataToEndOfFile()
        let standardError = String(data: errorData, encoding: .utf8) ?? ""
        return ProcessOutcome(terminationStatus: process.terminationStatus, standardError: standardError)
    }
}
