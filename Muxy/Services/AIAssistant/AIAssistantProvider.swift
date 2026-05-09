import Foundation

enum AIAssistantProvider: String, CaseIterable, Identifiable, Codable {
    case claude
    case codex
    case opencode
    case custom

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .claude: "Claude Code"
        case .codex: "Codex"
        case .opencode: "OpenCode"
        case .custom: "Custom Command"
        }
    }

    var defaultExecutable: String {
        switch self {
        case .claude: "claude"
        case .codex: "codex"
        case .opencode: "opencode"
        case .custom: ""
        }
    }

    func builtInArguments(model: String?) -> [String] {
        switch self {
        case .claude:
            var args = ["-p", "--output-format", "text"]
            if let model, !model.isEmpty {
                args.append(contentsOf: ["--model", model])
            }
            return args
        case .codex:
            var args = ["exec", "--skip-git-repo-check"]
            if let model, !model.isEmpty {
                args.append(contentsOf: ["--model", model])
            }
            args.append("-")
            return args
        case .opencode:
            var args = ["run"]
            if let model, !model.isEmpty {
                args.append(contentsOf: ["--model", model])
            }
            return args
        case .custom:
            return []
        }
    }
}
