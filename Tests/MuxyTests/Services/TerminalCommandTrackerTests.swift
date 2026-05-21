import Foundation
import GhosttyKit
import Testing

@testable import Muxy

@MainActor
@Suite("TerminalCommandTracker")
struct TerminalCommandTrackerTests {
    @Test("Submits buffer on newline")
    func submitsOnNewline() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("npm run dev\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "npm run dev")
    }

    @Test("Submits buffer on carriage return")
    func submitsOnCarriageReturn() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("git status\r", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git status")
    }

    @Test("recordReturn submits current buffer")
    func recordReturnSubmitsBuffer() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("make build", paneID: pane)
        TerminalCommandTracker.shared.recordReturn(paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "make build")
    }

    @Test("DEL (0x7F) removes last character")
    func delRemovesLastCharacter() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("lss\u{7F}\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "ls")
    }

    @Test("Backspace byte (0x08) removes last character")
    func backspaceByteRemovesLastCharacter() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("lss\u{8}\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "ls")
    }

    @Test("recordBackspace removes last character")
    func recordBackspaceRemovesLastCharacter() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("gitt", paneID: pane)
        TerminalCommandTracker.shared.recordBackspace(paneID: pane)
        TerminalCommandTracker.shared.recordReturn(paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git")
    }

    @Test("Whitespace-only submission does not overwrite last command")
    func whitespaceOnlySubmissionIgnored() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane)
        TerminalCommandTracker.shared.recordText("   \n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git log")
    }

    @Test("Empty submission does not overwrite last command")
    func emptySubmissionIgnored() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("git status\n", paneID: pane)
        TerminalCommandTracker.shared.recordText("\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git status")
    }

    @Test("Tab completion candidate does not overwrite last command")
    func tabCompletionCandidateDoesNotOverwriteLastCommand() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("nvim main.swift\n", paneID: pane)
        TerminalCommandTracker.shared.confirmCommand(paneID: pane)
        TerminalCommandTracker.shared.recordText("nv\t/U\tLApxter\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "nvim main.swift")
    }

    @Test("Escape sequence candidate does not overwrite last command")
    func escapeSequenceCandidateDoesNotOverwriteLastCommand() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("git status\n", paneID: pane)
        TerminalCommandTracker.shared.confirmCommand(paneID: pane)
        TerminalCommandTracker.shared.recordText("nvim \u{1B}[Dfile.json\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git status")
    }

    @Test("Last command updates on each submission")
    func lastCommandUpdatesEachSubmission() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("git status\n", paneID: pane)
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git log")
    }

    @Test("Escaped path command is preserved")
    func escapedPathCommandIsPreserved() {
        let pane = UUID()
        let command = "nvim /Users/some\\ user/Library/Application\\ Support/some\\ file.json"
        TerminalCommandTracker.shared.recordText(command + "\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == command)
    }

    @Test("Submitted command overrides older confirmed command before shell confirmation")
    func submittedCommandOverridesOlderConfirmedCommand() {
        let pane = UUID()
        let command = "nvim /Users/some\\ user/Library/Application\\ Support/some\\ file.json"
        TerminalCommandTracker.shared.recordText("nvim\n", paneID: pane)
        TerminalCommandTracker.shared.confirmCommand(paneID: pane)
        TerminalCommandTracker.shared.recordText(command + "\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == command)
    }

    @Test("Shell command overrides input reconstructed before completion")
    func shellCommandOverridesInputReconstructedBeforeCompletion() {
        let pane = UUID()
        let command = "nvim /Users/some\\ user/Library/Application\\ Support/some\\ file.json"
        TerminalCommandTracker.shared.recordText("nvim /U", paneID: pane)
        TerminalCommandTracker.shared.recordReturn(paneID: pane)
        TerminalCommandTracker.shared.recordShellCommandCandidate(command, paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == command)
    }

    @Test("Shell title candidate with different command does not overwrite pending command")
    func shellTitleCandidateWithDifferentCommandDoesNotOverwritePendingCommand() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("nvim /U", paneID: pane)
        TerminalCommandTracker.shared.recordReturn(paneID: pane)
        TerminalCommandTracker.shared.recordShellCommandCandidate("~/project", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "nvim /U")
    }

    @Test("Multiple panes tracked independently")
    func multiplePanesAreIndependent() {
        let pane1 = UUID()
        let pane2 = UUID()
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane1)
        TerminalCommandTracker.shared.recordText("npm test\n", paneID: pane2)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane1) == "git log")
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane2) == "npm test")
    }

    @Test("removePane clears all tracking state")
    func removePaneClearsState() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane)
        TerminalCommandTracker.shared.removePane(pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == nil)
    }

    @Test("Unknown pane returns nil")
    func unknownPaneReturnsNil() {
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: UUID()) == nil)
    }

    @Test("Backspace on empty buffer is a no-op")
    func backspaceOnEmptyBufferIsNoop() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordBackspace(paneID: pane)
        TerminalCommandTracker.shared.recordReturn(paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == nil)
    }

    @Test("Pending command visible before PWD confirmation")
    func pendingCommandVisibleBeforeConfirmation() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("vim main.swift\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "vim main.swift")
    }

    @Test("confirmCommand promotes pending to confirmed")
    func confirmCommandPromotesPending() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("vim main.swift\n", paneID: pane)
        TerminalCommandTracker.shared.confirmCommand(paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "vim main.swift")
    }

    @Test("Confirmed command persists when new pending appears")
    func confirmedCommandPersistsWhenNewPendingAppears() {
        let pane = UUID()
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane)
        TerminalCommandTracker.shared.confirmCommand(paneID: pane)
        TerminalCommandTracker.shared.recordText("git status", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git log")
    }

    @Test("Secure input ON blocks Enter submission")
    func secureInputBlocksEnter() {
        let pane = UUID()
        TerminalCommandTracker.shared.setSecureInput(GHOSTTY_SECURE_INPUT_ON, paneID: pane)
        TerminalCommandTracker.shared.recordText("mysecretpassword\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == nil)
    }

    @Test("Secure input OFF restores tracking")
    func secureInputOffRestoresTracking() {
        let pane = UUID()
        TerminalCommandTracker.shared.setSecureInput(GHOSTTY_SECURE_INPUT_ON, paneID: pane)
        TerminalCommandTracker.shared.recordText("password\n", paneID: pane)
        TerminalCommandTracker.shared.setSecureInput(GHOSTTY_SECURE_INPUT_OFF, paneID: pane)
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git log")
    }

    @Test("Secure input TOGGLE flips state")
    func secureInputToggle() {
        let pane = UUID()
        TerminalCommandTracker.shared.setSecureInput(GHOSTTY_SECURE_INPUT_TOGGLE, paneID: pane)
        TerminalCommandTracker.shared.recordText("blocked\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == nil)
        TerminalCommandTracker.shared.setSecureInput(GHOSTTY_SECURE_INPUT_TOGGLE, paneID: pane)
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git log")
    }

    @Test("removePane clears secure input state")
    func removePaneClearsSecureInputState() {
        let pane = UUID()
        TerminalCommandTracker.shared.setSecureInput(GHOSTTY_SECURE_INPUT_ON, paneID: pane)
        TerminalCommandTracker.shared.removePane(pane)
        TerminalCommandTracker.shared.recordText("git log\n", paneID: pane)
        #expect(TerminalCommandTracker.shared.lastSubmittedCommand(for: pane) == "git log")
    }
}
