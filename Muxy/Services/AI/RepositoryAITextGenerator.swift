import Foundation

enum RepositoryAITextGeneratorError: LocalizedError, Equatable {
    case failedToLaunch(String)
    case providerFailed(String, String)
    case emptyResponse(String)
    case timedOut(String)

    var errorDescription: String? {
        switch self {
        case let .failedToLaunch(message):
            message
        case let .providerFailed(provider, message):
            "\(provider) failed: \(message)"
        case let .emptyResponse(provider):
            "\(provider) returned an empty response."
        case let .timedOut(provider):
            "\(provider) did not respond within five minutes."
        }
    }
}

struct RepositoryAITextGenerator {
    private static let outputByteLimit = 262_144
    private let timeout: Duration

    init(timeout: Duration = .seconds(300)) {
        self.timeout = timeout
    }

    func generate(
        prompt: String,
        configuration: AIAgentLaunchConfiguration,
        providerName: String,
        workingDirectory: String,
        context: WorkspaceContext
    ) async throws -> String {
        guard let invocation = configuration.invocation(prompt: prompt) else {
            throw RepositoryAIMetadataError.invalidResponse
        }
        let launch = resolvedLaunch(
            invocation: invocation,
            workingDirectory: workingDirectory,
            context: context
        )
        let result: GitProcessResult
        do {
            result = try await run(launch, providerName: providerName)
        } catch let error as RepositoryAITextGeneratorError {
            throw error
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            throw RepositoryAITextGeneratorError.failedToLaunch(error.localizedDescription)
        }
        try Task.checkCancellation()
        guard !result.truncated else {
            throw RepositoryAITextGeneratorError.providerFailed(
                providerName,
                "The response exceeded the 256 KB output limit."
            )
        }
        guard result.status == 0 else {
            let detail = Self.failureDetail(stdout: result.stdout, stderr: result.stderr)
            throw RepositoryAITextGeneratorError.providerFailed(providerName, detail)
        }
        let response = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !response.isEmpty else {
            throw RepositoryAITextGeneratorError.emptyResponse(providerName)
        }
        return response
    }

    private func run(
        _ launch: ResolvedLaunch,
        providerName: String
    ) async throws -> GitProcessResult {
        try await withThrowingTaskGroup(of: GitProcessResult.self) { group in
            group.addTask {
                try await GitProcessRunner.runResolved(
                    launch,
                    outputByteLimit: Self.outputByteLimit
                )
            }
            group.addTask {
                try await Task.sleep(for: timeout)
                throw RepositoryAITextGeneratorError.timedOut(providerName)
            }
            defer { group.cancelAll() }
            guard let result = try await group.next() else {
                throw RepositoryAITextGeneratorError.timedOut(providerName)
            }
            return result
        }
    }

    func resolvedLaunch(
        invocation: AIAgentInvocation,
        workingDirectory: String,
        context: WorkspaceContext
    ) -> ResolvedLaunch {
        var environment = invocation.environment
        environment["MUXY_PANE_ID"] = ""
        guard !context.isRemote else {
            return CommandTransform.resolve(
                executable: invocation.executable,
                arguments: invocation.arguments,
                workingDirectory: workingDirectory,
                environment: environment,
                in: context
            )
        }

        environment["PATH"] = LoginShellPath.current
        let assignments = environment.keys.sorted().map { "\($0)=\(environment[$0] ?? "")" }
        return ResolvedLaunch(
            executable: "/usr/bin/env",
            arguments: assignments + [invocation.executable] + invocation.arguments,
            workingDirectory: workingDirectory
        )
    }

    static func failureDetail(stdout: String, stderr: String) -> String {
        let source = stderr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? stdout : stderr
        let sanitized = source.unicodeScalars
            .filter {
                !CharacterSet.controlCharacters.contains($0) || $0.value == 10 || $0.value == 9
            }
            .map(String.init)
            .joined()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !sanitized.isEmpty else { return "The command exited without an error message." }
        return String(sanitized.suffix(1000))
    }
}
