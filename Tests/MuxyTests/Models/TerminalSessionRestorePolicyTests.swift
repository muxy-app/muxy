import Foundation
import Testing
@testable import Muxy

@Suite("TerminalSessionRestorePolicy", .serialized)
struct TerminalSessionRestorePolicyTests {
    @Test("Allows commands that are not blocked")
    func allowsCommandsThatAreNotBlocked() {
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("nvim Package.swift"))
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("lazygit"))
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("npm run dev"))
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("ssh user@example.com"))
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("custom-tool --watch"))
    }

    @Test("Blocks excluded commands across shell segments")
    func blocksDangerousCommands() {
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("rm -rf build"))
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("git push --force origin main"))
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("npm run dev && rm -rf build"))
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("sudo npm run dev"))
    }

    @Test("Blocks commands that are prefixes of excluded patterns")
    func blocksPrefixExcludedCommands() {
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("rm file.txt"))
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("sudo apt install vim"))
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("git push -f origin main"))
    }

    @Test("Does not block commands that merely contain an excluded word")
    func doesNotBlockCommandsContainingExcludedWord() {
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("git log --oneline"))
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("echo 'no rm here'"))
    }

    @Test("Empty and whitespace-only commands are not safe")
    func emptyCommandIsNotSafe() {
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore(""))
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("   "))
    }

    @Test("Blocks excluded command in any shell segment of a pipeline")
    func blocksExcludedCommandInPipeline() {
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("find . -name '*.log' | sudo tee /dev/null"))
        #expect(!TerminalSessionRestorePolicy.isSafeToRestore("ls build; rm -rf build"))
    }

    @Test("Does not split on shell separators inside quoted strings")
    func doesNotSplitInsideQuotedStrings() {
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("echo \"safe | pipe\""))
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("echo 'safe; semicolon'"))
        #expect(TerminalSessionRestorePolicy.isSafeToRestore("grep -E 'foo|bar' file.txt"))
    }

    @Test("commandToRestore returns startupCommand when set")
    func commandToRestoreReturnsStartupCommand() {
        let snapshot = makeSnapshot(startupCommand: "npm run dev", lastSubmittedCommand: "git log", activity: .idle)
        #expect(snapshot.commandToRestore == "npm run dev")
    }

    @Test("commandToRestore ignores whitespace-only startupCommand")
    func commandToRestoreIgnoresWhitespaceStartupCommand() {
        let snapshot = makeSnapshot(startupCommand: "   ", lastSubmittedCommand: "git log", activity: .running)
        #expect(snapshot.commandToRestore == "git log")
    }

    @Test("commandToRestore returns nil for idle pane with no startupCommand")
    func commandToRestoreNilForIdlePaneWithNoStartupCommand() {
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: "git log", activity: .idle)
        #expect(snapshot.commandToRestore == nil)
    }

    @Test("commandToRestore returns lastSubmittedCommand for running pane")
    func commandToRestoreReturnsLastSubmittedForRunningPane() {
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: "claude", activity: .running)
        #expect(snapshot.commandToRestore == "claude")
    }

    @Test("commandToRestore returns nil for running pane with no lastSubmittedCommand")
    func commandToRestoreNilForRunningPaneWithNoCommand() {
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: nil, activity: .running)
        #expect(snapshot.commandToRestore == nil)
    }

    @Test("decision returns command when safe and enabled")
    func decisionReturnsCommandWhenSafe() {
        SessionRestorePreferences.isEnabled = true
        defer { UserDefaults.standard.removeObject(forKey: SessionRestorePreferences.enabledKey) }
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: "nvim main.swift", activity: .running)
        #expect(TerminalSessionRestorePolicy.decision(for: snapshot) == .command("nvim main.swift"))
    }

    @Test("decision preserves quoted paths with spaces")
    func decisionPreservesQuotedPathsWithSpaces() {
        SessionRestorePreferences.isEnabled = true
        defer { UserDefaults.standard.removeObject(forKey: SessionRestorePreferences.enabledKey) }
        let command = "nvim '/Users/some user/Library/Application Support/some file.json'"
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: command, activity: .running)
        #expect(TerminalSessionRestorePolicy.decision(for: snapshot) == .command(command))
    }

    @Test("decision returns none when feature disabled")
    func decisionNoneWhenDisabled() {
        SessionRestorePreferences.isEnabled = false
        defer { UserDefaults.standard.removeObject(forKey: SessionRestorePreferences.enabledKey) }
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: "nvim main.swift", activity: .running)
        #expect(TerminalSessionRestorePolicy.decision(for: snapshot) == .none)
    }

    @Test("decision returns none for blocked command")
    func decisionNoneForBlockedCommand() {
        SessionRestorePreferences.isEnabled = true
        defer { UserDefaults.standard.removeObject(forKey: SessionRestorePreferences.enabledKey) }
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: "sudo rm -rf /", activity: .running)
        #expect(TerminalSessionRestorePolicy.decision(for: snapshot) == .none)
    }

    @Test("decision preserves AI tool command")
    func decisionPreservesAIToolCommand() {
        SessionRestorePreferences.isEnabled = true
        defer { UserDefaults.standard.removeObject(forKey: SessionRestorePreferences.enabledKey) }
        let snapshot = makeSnapshot(startupCommand: nil, lastSubmittedCommand: "claude", activity: .running)
        #expect(TerminalSessionRestorePolicy.decision(for: snapshot) == .command("claude"))
    }
}

private func makeSnapshot(
    startupCommand: String?,
    lastSubmittedCommand: String?,
    activity: TerminalSessionSnapshot.Activity
) -> TerminalSessionSnapshot {
    TerminalSessionSnapshot(
        id: UUID(),
        projectID: UUID(),
        worktreeID: UUID(),
        paneID: UUID(),
        tabID: UUID(),
        areaID: UUID(),
        projectPath: "/tmp/project",
        title: "Terminal",
        workingDirectory: "/tmp/project",
        startupCommand: startupCommand,
        lastSubmittedCommand: lastSubmittedCommand,
        activity: activity,
        capturedAt: Date()
    )
}
