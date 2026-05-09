import Foundation
import Testing

@testable import Muxy

@Suite("AIAssistantService")
struct AIAssistantServiceTests {
    @Test("cleanProviderOutput trims whitespace")
    func cleanTrimsWhitespace() {
        #expect(AIAssistantService.cleanProviderOutput("\n  fix bug\n\n") == "fix bug")
    }

    @Test("cleanProviderOutput strips ``` code fences")
    func cleanStripsBareFence() {
        let raw = "```\nfix: thing\n\nbody line\n```"
        #expect(AIAssistantService.cleanProviderOutput(raw) == "fix: thing\n\nbody line")
    }

    @Test("cleanProviderOutput strips language-tagged code fences")
    func cleanStripsLanguageFence() {
        let raw = "```text\nsubject\n```"
        #expect(AIAssistantService.cleanProviderOutput(raw) == "subject")
    }

    @Test("cleanProviderOutput leaves non-fenced text alone")
    func cleanLeavesPlainAlone() {
        let raw = "subject line\n\nbody"
        #expect(AIAssistantService.cleanProviderOutput(raw) == "subject line\n\nbody")
    }

    @Test("extractJSONObject finds first balanced object")
    func extractFirstObject() {
        let text = "preamble {\"title\": \"x\", \"body\": \"y\"} trailing"
        #expect(AIAssistantService.extractJSONObject(from: text) == "{\"title\": \"x\", \"body\": \"y\"}")
    }

    @Test("extractJSONObject handles nested braces")
    func extractNested() {
        let text = "{\"title\": \"a\", \"body\": \"line {nested} done\"}"
        #expect(AIAssistantService.extractJSONObject(from: text) == text)
    }

    @Test("extractJSONObject ignores braces inside strings")
    func extractIgnoresBracesInStrings() {
        let text = "{\"body\": \"open { brace\"}"
        #expect(AIAssistantService.extractJSONObject(from: text) == text)
    }

    @Test("extractJSONObject handles escaped quotes")
    func extractEscapedQuotes() {
        let text = "{\"body\": \"with \\\" quote\"}"
        #expect(AIAssistantService.extractJSONObject(from: text) == text)
    }

    @Test("extractJSONObject returns nil when no object present")
    func extractMissing() {
        #expect(AIAssistantService.extractJSONObject(from: "no json here") == nil)
    }

    @Test("parsePullRequest extracts title and body")
    func parseValid() throws {
        let raw = "```json\n{\"title\": \"Fix crash\", \"body\": \"Avoid blocking DNS.\"}\n```"
        let draft = try AIAssistantService.parsePullRequest(raw)
        #expect(draft.title == "Fix crash")
        #expect(draft.body == "Avoid blocking DNS.")
    }

    @Test("parsePullRequest tolerates surrounding prose")
    func parseSurroundingProse() throws {
        let raw = "Sure! Here's the PR:\n{\"title\": \"x\", \"body\": \"y\"}\nLet me know."
        let draft = try AIAssistantService.parsePullRequest(raw)
        #expect(draft.title == "x")
        #expect(draft.body == "y")
    }

    @Test("parsePullRequest fails when title missing")
    func parseMissingTitle() {
        let raw = "{\"body\": \"only body\"}"
        #expect(throws: AIAssistantRunnerError.self) {
            try AIAssistantService.parsePullRequest(raw)
        }
    }

    @Test("parsePullRequest fails on garbage input")
    func parseGarbage() {
        #expect(throws: AIAssistantRunnerError.self) {
            try AIAssistantService.parsePullRequest("not json at all")
        }
    }
}

@Suite("AIAssistantPrompts")
struct AIAssistantPromptsTests {
    @Test("composedPrompt includes branch context when provided")
    func composedWithBranches() {
        let prompt = AIAssistantPrompts.composedPrompt(
            for: .pullRequest,
            userPrompt: "Write something.",
            diff: "diff body",
            branch: "feature/x",
            baseBranch: "main"
        )
        #expect(prompt.contains("Current branch: feature/x"))
        #expect(prompt.contains("Base branch: main"))
        #expect(prompt.contains("Diff:\ndiff body"))
        #expect(prompt.contains("Write something."))
    }

    @Test("composedPrompt omits empty branch lines")
    func composedSkipsEmptyBranches() {
        let prompt = AIAssistantPrompts.composedPrompt(
            for: .commitMessage,
            userPrompt: "Write a commit.",
            diff: "diff",
            branch: nil,
            baseBranch: nil
        )
        #expect(!prompt.contains("Current branch:"))
        #expect(!prompt.contains("Base branch:"))
    }

    @Test("composedPrompt trims user prompt whitespace")
    func composedTrimsUserPrompt() {
        let prompt = AIAssistantPrompts.composedPrompt(
            for: .commitMessage,
            userPrompt: "\n  Trim me.  \n",
            diff: "d",
            branch: nil,
            baseBranch: nil
        )
        #expect(prompt.contains("Trim me."))
        #expect(!prompt.contains("\n  Trim me."))
    }
}

@Suite("AIAssistantRunner")
struct AIAssistantRunnerTests {
    @Test("firstToken returns command word")
    func firstTokenSimple() {
        #expect(AIAssistantRunner.firstToken("mytool --flag value") == "mytool")
    }

    @Test("firstToken returns whole string when no whitespace")
    func firstTokenNoWhitespace() {
        #expect(AIAssistantRunner.firstToken("solo") == "solo")
    }

    @Test("isCommandNotFound matches zsh and bash phrasing")
    func detectsCommandNotFound() {
        #expect(AIAssistantRunner.isCommandNotFound(stderr: "zsh: command not found: claude"))
        #expect(AIAssistantRunner.isCommandNotFound(stderr: "bash: claude: command not found"))
        #expect(AIAssistantRunner.isCommandNotFound(stderr: "No such file or directory"))
        #expect(!AIAssistantRunner.isCommandNotFound(stderr: "API rate limit exceeded"))
    }

    @Test("resolveInvocation throws when custom command empty")
    func customEmptyThrows() {
        #expect(throws: AIAssistantRunnerError.self) {
            try AIAssistantRunner.resolveInvocation(provider: .custom, customCommand: "   ", model: nil)
        }
    }

    @Test("resolveInvocation builds login-shell args for custom")
    func customBuildsShellArgs() throws {
        let invocation = try AIAssistantRunner.resolveInvocation(
            provider: .custom,
            customCommand: "mytool --quiet",
            model: nil
        )
        #expect(invocation.usesLoginShell)
        #expect(invocation.arguments == ["-l", "-c", "mytool --quiet"])
        #expect(invocation.displayName == "mytool")
    }
}
