import Foundation

struct AIAgentInvocation: Equatable {
    let executable: String
    let arguments: [String]
    let environment: [String: String]
}

struct AIAgentLaunchConfiguration: Equatable {
    let executable: String
    let headlessArguments: [String]
    let modelArgument: String?
    let environment: [String: String]

    init(
        executable: String,
        headlessArguments: [String],
        modelArgument: String? = "--model",
        environment: [String: String] = [:]
    ) {
        self.executable = executable
        self.headlessArguments = headlessArguments
        self.modelArgument = modelArgument
        self.environment = environment
    }

    func invocation(prompt: String, model: String? = nil) -> AIAgentInvocation? {
        let prompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty else { return nil }

        var arguments = headlessArguments
        if let modelArgument,
           let model = model?.trimmingCharacters(in: .whitespacesAndNewlines),
           !model.isEmpty
        {
            arguments.append(contentsOf: [modelArgument, model])
        }
        arguments.append(prompt.first == "-" ? " \(prompt)" : prompt)
        return AIAgentInvocation(
            executable: executable,
            arguments: arguments,
            environment: environment
        )
    }
}

protocol AIAgentLaunchProvider {
    var id: String { get }
    var displayName: String { get }
    var iconName: String { get }
    var agentLaunchConfiguration: AIAgentLaunchConfiguration { get }

    func agentCLIExecutablePath() -> String?
    func isAgentCLIInstalled() -> Bool
}

extension AIAgentLaunchProvider {
    func agentCLIExecutablePath() -> String? {
        ProviderExecutableLocator.executablePath(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: NSHomeDirectory(),
            pathEnvironment: LoginShellPath.current,
            includeSystemWide: true
        )
    }

    func isAgentCLIInstalled() -> Bool {
        agentCLIExecutablePath() != nil
    }
}

enum AgentTabLaunchCommand {
    static func local(provider: any AIAgentLaunchProvider) -> String? {
        provider.agentCLIExecutablePath().map(ShellEscaper.escape)
    }

    static func remote(provider: any AIAgentLaunchProvider) -> String {
        ShellEscaper.escape(provider.agentLaunchConfiguration.executable)
    }
}

struct AgentTabLaunchOption: Identifiable {
    let provider: any AIAgentLaunchProvider
    let command: String?

    var id: String { provider.id }

    var title: String {
        command == nil ? "\(provider.displayName) · Not installed" : provider.displayName
    }

    static func resolveLocal(providers: [any AIAgentLaunchProvider]) -> [Self] {
        providers.map { provider in
            Self(
                provider: provider,
                command: AgentTabLaunchCommand.local(provider: provider)
            )
        }
    }

    static func resolveRemote(
        providers: [any AIAgentLaunchProvider],
        availableProviderIDs: Set<String>
    ) -> [Self] {
        providers.map { provider in
            Self(
                provider: provider,
                command: availableProviderIDs.contains(provider.id)
                    ? AgentTabLaunchCommand.remote(provider: provider)
                    : nil
            )
        }
    }

    @MainActor
    static func resolveLocal() -> [Self] {
        resolveLocal(providers: AIProviderRegistry.shared.agentLaunchProviders)
    }

    @MainActor
    static func resolveRemote(destination: SSHDestination) async throws -> [Self] {
        let providers = AIProviderRegistry.shared.agentLaunchProviders
        let availableProviderIDs = try await RemoteAgentLaunchAvailability.resolve(
            providers: providers,
            destination: destination
        )
        return resolveRemote(providers: providers, availableProviderIDs: availableProviderIDs)
    }
}

@MainActor
enum AgentTabLaunchOptionsLoader {
    typealias LocalResolver = @MainActor () -> [AgentTabLaunchOption]
    typealias RemoteResolver = @MainActor (SSHDestination) async throws -> [AgentTabLaunchOption]

    static func resolve(
        context: WorkspaceContext,
        localResolver: LocalResolver = { AgentTabLaunchOption.resolveLocal() },
        remoteResolver: RemoteResolver = { destination in
            try await AgentTabLaunchOption.resolveRemote(destination: destination)
        }
    ) async throws -> [AgentTabLaunchOption] {
        let options: [AgentTabLaunchOption] = switch context {
        case .local:
            localResolver()
        case let .ssh(destination):
            try await remoteResolver(destination)
        }
        try Task.checkCancellation()
        return options
    }
}

enum RemoteAgentLaunchAvailabilityError: LocalizedError {
    case commandFailed(String)

    var errorDescription: String? {
        switch self {
        case let .commandFailed(message):
            message.isEmpty ? "Could not check remote agent providers." : message
        }
    }
}

enum RemoteAgentLaunchAvailability {
    typealias Runner = @MainActor (SSHDestination, String) async throws -> GitProcessResult

    private static let startMarker = "__MUXY_AGENT_PROVIDERS_START__"
    private static let endMarker = "__MUXY_AGENT_PROVIDERS_END__"

    @MainActor
    static func resolve(
        providers: [any AIAgentLaunchProvider],
        destination: SSHDestination,
        runner: Runner = { destination, command in
            try await SSHCommandRunner.run(destination: destination, remoteCommand: command)
        }
    ) async throws -> Set<String> {
        let result = try await runner(destination, lookupCommand(providers: providers))
        guard result.status == 0 else {
            throw RemoteAgentLaunchAvailabilityError.commandFailed(
                result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        }
        let supportedProviderIDs = Set(providers.map(\.id))
        return providerIDs(from: result.stdout).intersection(supportedProviderIDs)
    }

    static func lookupCommand(providers: [any AIAgentLaunchProvider]) -> String {
        let checks = providers.map { provider in
            let executable = ShellEscaper.escape(provider.agentLaunchConfiguration.executable)
            let providerID = ShellEscaper.escape(provider.id)
            return "if command -v \(executable) >/dev/null 2>&1; then printf '%s\\n' \(providerID); fi"
        }
        let script = ([
            "printf '%s\\n' \(ShellEscaper.escape(startMarker))",
        ] + checks + [
            "printf '%s\\n' \(ShellEscaper.escape(endMarker))",
        ]).joined(separator: "; ")
        return "exec \"${SHELL:-/bin/sh}\" -l -i -c \(ShellEscaper.escape(script))"
    }

    static func providerIDs(from output: String) -> Set<String> {
        let lines = output.split(whereSeparator: \.isNewline).map {
            $0.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        guard let start = lines.firstIndex(of: startMarker),
              let end = lines[(start + 1)...].firstIndex(of: endMarker)
        else { return [] }
        return Set(lines[(start + 1) ..< end])
    }
}
