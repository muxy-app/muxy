import Foundation

enum AIAssistantTask {
    case commitMessage
    case pullRequest
}

enum AIAssistantPrompts {
    static let defaultCommitUserPrompt = """
    Write a concise, conventional git commit message for the staged changes below.
    - Subject line in imperative mood, max 72 characters.
    - Optional body, wrapped at 72 characters, explaining the why.
    - Do not include code fences, quotes, or any extra commentary.
    """

    static let defaultPullRequestUserPrompt = """
    Write a pull request title and short description for the diff below.
    - Title: short, imperative, max 70 characters.
    - Description: 1-3 short bullet points or sentences focused on the why.
    - Do not include code fences or extra commentary.
    """

    static func systemPrompt(for task: AIAssistantTask) -> String {
        switch task {
        case .commitMessage:
            """
            You are an assistant that generates git commit messages.
            Output ONLY the commit message in plain text. No code fences, no preamble, no JSON.
            Subject line first, then a blank line, then optional body. No trailing notes.
            """
        case .pullRequest:
            """
            You are an assistant that generates pull request metadata.
            Output ONLY a single JSON object with exactly two string keys: "title" and "body".
            No code fences, no preamble, no trailing text. Example:
            {"title": "Fix crash on launch", "body": "Avoid blocking DNS by removing hostName lookup."}
            """
        }
    }

    static func composedPrompt(
        for task: AIAssistantTask,
        userPrompt: String,
        diff: String,
        branch: String?,
        baseBranch: String?
    ) -> String {
        let trimmedUser = userPrompt.trimmingCharacters(in: .whitespacesAndNewlines)
        var sections: [String] = [systemPrompt(for: task), trimmedUser]

        var contextLines: [String] = []
        if let branch, !branch.isEmpty {
            contextLines.append("Current branch: \(branch)")
        }
        if let baseBranch, !baseBranch.isEmpty {
            contextLines.append("Base branch: \(baseBranch)")
        }
        if !contextLines.isEmpty {
            sections.append(contextLines.joined(separator: "\n"))
        }

        sections.append("Diff:\n\(diff)")
        return sections.joined(separator: "\n\n")
    }
}
