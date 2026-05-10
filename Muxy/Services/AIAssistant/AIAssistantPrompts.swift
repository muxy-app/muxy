import Foundation

enum AIAssistantTask {
    case commitMessage
    case pullRequest
}

enum AIAssistantPrompts {
    static let defaultCommitUserPrompt = """
    Write the commit message for the staged changes below.

    How to read the diff:
    1. Group hunks by the underlying change they implement (one diff often
       contains one logical change spread across many files).
    2. Identify the dominant change. Treat refactors, renames, test updates,
       and formatting as supporting context unless they ARE the change.
    3. If you see unrelated upstream changes (e.g. from a recent merge of the
       base branch), ignore them and focus only on this commit's intent.

    Then write the message per the system prompt's format.
    """

    static let defaultPullRequestUserPrompt = """
    Draft a PR title and body for the diff below.

    Diff scope:
    - The committed section is this branch's changes relative to the
      merge-base with the base branch. Commits already on the base branch
      are NOT included.
    - If the branch was recently merged with the base, you may still see
      hunks that look like upstream code (imports moved, files reformatted
      by another PR, lockfile churn). Ignore anything that does not match
      this branch's intent. When in doubt, prefer narrower claims.
    - The working-tree section, when present, lists uncommitted changes.
      Treat them as part of the same PR.

    How to read the diff:
    1. Group hunks by the underlying change.
    2. Pick the single dominant outcome - that is the title.
    3. The body explains why it matters and any reviewer-relevant context.

    Output the JSON object only.
    """

    static func systemPrompt(for task: AIAssistantTask) -> String {
        switch task {
        case .commitMessage:
            """
            You are a senior engineer writing a git commit message for a teammate.
            Goals: explain the WHY, not the WHAT. The diff already shows the what.

            Output format (STRICT):
            - Plain text only. No code fences, no preamble, no trailing notes.
            - Subject line: imperative mood, <=72 chars, no trailing period, no
              scope prefix unless the repo already uses Conventional Commits.
            - Then a blank line.
            - Optional body: wrapped at 72 chars, focused on motivation,
              trade-offs, or non-obvious mechanics. Skip the body for trivial
              changes.

            Never:
            - Restate the file list or function names already visible in the diff.
            - Begin with "This commit", "Updated", "Changed" - start with the action.
            - Mention tests, lint, or formatting unless they are the actual subject.
            """
        case .pullRequest:
            """
            You are a senior engineer drafting a pull request for review.
            Goals: a reviewer should know in 10 seconds what this PR does and why.

            Output format (STRICT):
            - A single JSON object with exactly two string keys: "title" and "body".
            - No code fences, no preamble, no trailing text. The response must
              parse as JSON.

            Title rules:
            - Imperative mood, <=70 chars, no trailing period.
            - Describes the outcome, not the mechanism. ("Fix crash on launch",
              not "Remove blocking DNS call".)

            Body rules:
            - 1-3 short bullets OR 1-2 short sentences. Markdown allowed.
            - Focus on WHY and user-visible impact. Skip file lists.
            - If the change has a non-obvious risk, migration, or follow-up,
              name it in one final line.

            Example:
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
