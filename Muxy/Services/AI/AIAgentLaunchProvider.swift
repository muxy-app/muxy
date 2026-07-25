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
    static func resolve(provider: any AIAgentLaunchProvider, isRemote: Bool) -> String? {
        let executable = if isRemote {
            provider.agentLaunchConfiguration.executable
        } else {
            provider.agentCLIExecutablePath()
        }
        return executable.map(ShellEscaper.escape)
    }
}

struct AgentTabLaunchOption: Identifiable {
    let provider: any AIAgentLaunchProvider
    let command: String?

    var id: String { provider.id }

    var title: String {
        command == nil ? "\(provider.displayName) · Not installed" : provider.displayName
    }

    static func resolve(providers: [any AIAgentLaunchProvider], isRemote: Bool) -> [Self] {
        providers.map { provider in
            Self(
                provider: provider,
                command: AgentTabLaunchCommand.resolve(provider: provider, isRemote: isRemote)
            )
        }
    }
}
