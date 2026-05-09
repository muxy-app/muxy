import Foundation

struct AIAssistantSettingsSnapshot {
    let provider: AIAssistantProvider
    let claudeModel: String?
    let codexModel: String?
    let opencodeModel: String?
    let customCommand: String
    let commitPrompt: String?
    let prPrompt: String?

    func model(for provider: AIAssistantProvider) -> String? {
        switch provider {
        case .claude: claudeModel
        case .codex: codexModel
        case .opencode: opencodeModel
        case .custom: nil
        }
    }

    func userPrompt(for task: AIAssistantTask) -> String {
        switch task {
        case .commitMessage:
            commitPrompt ?? AIAssistantPrompts.defaultCommitUserPrompt
        case .pullRequest:
            prPrompt ?? AIAssistantPrompts.defaultPullRequestUserPrompt
        }
    }
}

enum AIAssistantSettings {
    static let providerKey = "muxy.ai.assistant.provider"
    static let claudeModelKey = "muxy.ai.assistant.model.claude"
    static let codexModelKey = "muxy.ai.assistant.model.codex"
    static let opencodeModelKey = "muxy.ai.assistant.model.opencode"
    static let customCommandKey = "muxy.ai.assistant.customCommand"
    static let commitPromptKey = "muxy.ai.assistant.prompt.commit"
    static let prPromptKey = "muxy.ai.assistant.prompt.pr"

    static func snapshot() -> AIAssistantSettingsSnapshot {
        let defaults = UserDefaults.standard
        let providerRaw = defaults.string(forKey: providerKey) ?? AIAssistantProvider.claude.rawValue
        let provider = AIAssistantProvider(rawValue: providerRaw) ?? .claude
        return AIAssistantSettingsSnapshot(
            provider: provider,
            claudeModel: trimmed(defaults.string(forKey: claudeModelKey)),
            codexModel: trimmed(defaults.string(forKey: codexModelKey)),
            opencodeModel: trimmed(defaults.string(forKey: opencodeModelKey)),
            customCommand: defaults.string(forKey: customCommandKey) ?? "",
            commitPrompt: trimmed(defaults.string(forKey: commitPromptKey)),
            prPrompt: trimmed(defaults.string(forKey: prPromptKey))
        )
    }

    private static func trimmed(_ value: String?) -> String? {
        let value = value?.trimmingCharacters(in: .whitespacesAndNewlines)
        return (value?.isEmpty ?? true) ? nil : value
    }
}
