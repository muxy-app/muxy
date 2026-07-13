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

    func isAgentCLIInstalled() -> Bool
}

extension AIAgentLaunchProvider where Self: AIProviderIntegration {
    func isAgentCLIInstalled() -> Bool {
        isToolInstalled() || ProviderExecutableLocator.isInstalled(
            names: [agentLaunchConfiguration.executable],
            homeDirectory: NSHomeDirectory(),
            pathEnvironment: LoginShellPath.current,
            includeSystemWide: true
        )
    }
}
