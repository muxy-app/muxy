import Foundation

enum DiffAgentPrompt {
    static func build(
        comments: [DiffInlineComment],
        rowsProvider: (DiffInlineComment) -> [DiffDisplayRow]
    ) -> String {
        var sections: [String] = [
            "Address the following review comments on the current branch changes. "
                + "For each comment, make the requested change in the referenced file and lines.",
        ]
        for (index, comment) in comments.enumerated() {
            let snippet = DiffCommentAnchor.snippet(for: comment, in: rowsProvider(comment))
            var lines = [
                "## Comment \(index + 1)",
                "File: \(comment.filePath)",
                "Lines: \(comment.lineRangeLabel) (\(comment.side == .old ? "original" : "new") side)",
            ]
            if !snippet.isEmpty {
                lines.append("Code:")
                lines.append("```")
                lines.append(snippet)
                lines.append("```")
            }
            lines.append("Comment: \(comment.body)")
            sections.append(lines.joined(separator: "\n"))
        }
        return sections.joined(separator: "\n\n")
    }
}

enum DiffAgentLauncher {
    static func command(for provider: AIAssistantProvider, settings: AIAssistantSettingsSnapshot, prompt: String) -> String? {
        guard let invocation = executableInvocation(for: provider, settings: settings) else { return nil }
        let heredoc = "$(cat <<'MUXY_PROMPT_EOF'\n\(prompt)\nMUXY_PROMPT_EOF\n)"
        return "\(invocation) \(shellSingleQuoted(heredoc, isCommandSubstitution: true))"
    }

    private static func executableInvocation(for provider: AIAssistantProvider, settings: AIAssistantSettingsSnapshot) -> String? {
        if provider == .custom {
            let trimmed = settings.customCommand.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        }
        var parts = [provider.defaultExecutable]
        if let model = settings.model(for: provider), !model.isEmpty {
            parts.append(contentsOf: ["--model", model])
        }
        return parts.joined(separator: " ")
    }

    private static func shellSingleQuoted(_ value: String, isCommandSubstitution: Bool) -> String {
        guard isCommandSubstitution else {
            return "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'"
        }
        return "\"" + value + "\""
    }
}
