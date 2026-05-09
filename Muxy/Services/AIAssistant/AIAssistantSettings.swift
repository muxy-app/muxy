import Foundation

enum AIAssistantSettings {
    static let providerKey = "muxy.ai.assistant.provider"
    static let claudeModelKey = "muxy.ai.assistant.model.claude"
    static let codexModelKey = "muxy.ai.assistant.model.codex"
    static let opencodeModelKey = "muxy.ai.assistant.model.opencode"
    static let customCommandKey = "muxy.ai.assistant.customCommand"
    static let commitPromptKey = "muxy.ai.assistant.prompt.commit"
    static let prPromptKey = "muxy.ai.assistant.prompt.pr"

    static var provider: AIAssistantProvider {
        let raw = UserDefaults.standard.string(forKey: providerKey) ?? AIAssistantProvider.claude.rawValue
        return AIAssistantProvider(rawValue: raw) ?? .claude
    }

    static func model(for provider: AIAssistantProvider) -> String? {
        let key = modelKey(for: provider) ?? ""
        let value = UserDefaults.standard.string(forKey: key)?.trimmingCharacters(in: .whitespacesAndNewlines)
        return (value?.isEmpty ?? true) ? nil : value
    }

    static func modelKey(for provider: AIAssistantProvider) -> String? {
        switch provider {
        case .claude: claudeModelKey
        case .codex: codexModelKey
        case .opencode: opencodeModelKey
        case .custom: nil
        }
    }

    static var customCommand: String {
        UserDefaults.standard.string(forKey: customCommandKey) ?? ""
    }

    static func userPrompt(for task: AIAssistantTask) -> String {
        let key = task == .commitMessage ? commitPromptKey : prPromptKey
        let stored = UserDefaults.standard.string(forKey: key)?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let stored, !stored.isEmpty {
            return stored
        }
        return defaultUserPrompt(for: task)
    }

    static func defaultUserPrompt(for task: AIAssistantTask) -> String {
        switch task {
        case .commitMessage: AIAssistantPrompts.defaultCommitUserPrompt
        case .pullRequest: AIAssistantPrompts.defaultPullRequestUserPrompt
        }
    }
}
