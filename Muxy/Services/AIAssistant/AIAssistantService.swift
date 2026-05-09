import Foundation

struct AIPullRequestDraft {
    let title: String
    let body: String
}

enum AIAssistantServiceError: Error, LocalizedError {
    case noChanges
    case diffFailed(String)

    var errorDescription: String? {
        switch self {
        case .noChanges: "No changes to summarize."
        case let .diffFailed(message): "Failed to read git diff: \(message)"
        }
    }
}

@MainActor
enum AIAssistantService {
    private static let diffLineLimit = 4000
    private static let truncationMarker = "\n[diff truncated by Muxy at \(diffLineLimit) lines]\n"

    static func generateCommitMessage(
        repoPath: String,
        branch: String?
    ) async throws -> String {
        let settings = AIAssistantSettings.snapshot()
        let diff = try await stagedDiff(repoPath: repoPath)
        guard !diff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw AIAssistantServiceError.noChanges
        }
        let prompt = AIAssistantPrompts.composedPrompt(
            for: .commitMessage,
            userPrompt: settings.userPrompt(for: .commitMessage),
            diff: diff,
            branch: branch,
            baseBranch: nil
        )
        let raw = try await runProvider(prompt: prompt, repoPath: repoPath, settings: settings)
        return cleanCommitOutput(raw)
    }

    static func generatePullRequest(
        repoPath: String,
        branch: String?,
        baseBranch: String?
    ) async throws -> AIPullRequestDraft {
        let settings = AIAssistantSettings.snapshot()
        let diff = try await branchDiff(repoPath: repoPath, branch: branch, baseBranch: baseBranch)
        guard !diff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw AIAssistantServiceError.noChanges
        }
        let prompt = AIAssistantPrompts.composedPrompt(
            for: .pullRequest,
            userPrompt: settings.userPrompt(for: .pullRequest),
            diff: diff,
            branch: branch,
            baseBranch: baseBranch
        )
        let raw = try await runProvider(prompt: prompt, repoPath: repoPath, settings: settings)
        return try parsePullRequest(raw)
    }

    private static func runProvider(
        prompt: String,
        repoPath: String,
        settings: AIAssistantSettingsSnapshot
    ) async throws -> String {
        let invocation = try AIAssistantRunner.resolveInvocation(
            provider: settings.provider,
            customCommand: settings.customCommand,
            model: settings.model(for: settings.provider)
        )
        return try await AIAssistantRunner.run(
            invocation: invocation,
            prompt: prompt,
            workingDirectory: repoPath
        )
    }

    private static func stagedDiff(repoPath: String) async throws -> String {
        let staged = try await runDiff(repoPath: repoPath, arguments: ["diff", "--cached", "--no-color"])
        if !staged.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return staged
        }
        return try await runDiff(repoPath: repoPath, arguments: ["diff", "--no-color"])
    }

    private static func branchDiff(
        repoPath: String,
        branch: String?,
        baseBranch: String?
    ) async throws -> String {
        if let baseBranch, let branch, !baseBranch.isEmpty, !branch.isEmpty, baseBranch != branch {
            let range = "\(baseBranch)...\(branch)"
            let committed = try await runDiff(
                repoPath: repoPath,
                arguments: ["diff", "--no-color", range]
            )
            let working = try await runDiff(repoPath: repoPath, arguments: ["diff", "--no-color", "HEAD"])
            return [committed, working]
                .filter { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
                .joined(separator: "\n")
        }
        return try await runDiff(repoPath: repoPath, arguments: ["diff", "--no-color", "HEAD"])
    }

    private static func runDiff(repoPath: String, arguments: [String]) async throws -> String {
        do {
            let result = try await GitProcessRunner.runGit(
                repoPath: repoPath,
                arguments: arguments,
                lineLimit: diffLineLimit
            )
            if result.status != 0, !result.stderr.isEmpty {
                throw AIAssistantServiceError.diffFailed(result.stderr)
            }
            return result.truncated ? result.stdout + truncationMarker : result.stdout
        } catch let error as AIAssistantServiceError {
            throw error
        } catch {
            throw AIAssistantServiceError.diffFailed(error.localizedDescription)
        }
    }

    private static func cleanCommitOutput(_ raw: String) -> String {
        var text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("```") {
            text = stripCodeFence(text)
        }
        return text.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func stripCodeFence(_ text: String) -> String {
        var lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        if let first = lines.first, first.hasPrefix("```") {
            lines.removeFirst()
        }
        if let last = lines.last, last.trimmingCharacters(in: .whitespaces).hasPrefix("```") {
            lines.removeLast()
        }
        return lines.joined(separator: "\n")
    }

    private static func parsePullRequest(_ raw: String) throws -> AIPullRequestDraft {
        let cleaned = stripCodeFence(raw.trimmingCharacters(in: .whitespacesAndNewlines))
        guard let json = extractJSONObject(from: cleaned) else {
            throw AIAssistantRunnerError.parsingFailed("Provider response did not contain valid JSON.")
        }
        guard let data = json.data(using: .utf8) else {
            throw AIAssistantRunnerError.parsingFailed("Could not decode provider response.")
        }
        do {
            let object = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
            guard let dict = object as? [String: Any] else {
                throw AIAssistantRunnerError.parsingFailed("Expected a JSON object with title and body.")
            }
            let title = (dict["title"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            let body = (dict["body"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            if title.isEmpty {
                throw AIAssistantRunnerError.parsingFailed("Provider response missing 'title'.")
            }
            return AIPullRequestDraft(title: title, body: body)
        } catch let error as AIAssistantRunnerError {
            throw error
        } catch {
            throw AIAssistantRunnerError.parsingFailed("Invalid JSON in provider response.")
        }
    }

    private static func extractJSONObject(from text: String) -> String? {
        guard let start = text.firstIndex(of: "{") else { return nil }
        var depth = 0
        var inString = false
        var escape = false
        var index = start
        while index < text.endIndex {
            let char = text[index]
            if escape {
                escape = false
            } else if char == "\\" {
                escape = true
            } else if char == "\"" {
                inString.toggle()
            } else if !inString {
                if char == "{" {
                    depth += 1
                } else if char == "}" {
                    depth -= 1
                    if depth == 0 {
                        return String(text[start ... index])
                    }
                }
            }
            index = text.index(after: index)
        }
        return nil
    }
}
