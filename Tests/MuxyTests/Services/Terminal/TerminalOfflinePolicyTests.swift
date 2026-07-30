import Foundation
import Testing

@testable import Muxy

@Suite("TerminalOfflinePolicy")
struct TerminalOfflinePolicyTests {
    private func candidate(
        hasLiveSurface: Bool = true,
        isAlreadyOffline: Bool = false,
        invisibleDuration: TimeInterval? = 600,
        isIdle: Bool = true
    ) -> TerminalOfflinePolicy.Candidate {
        TerminalOfflinePolicy.Candidate(
            hasLiveSurface: hasLiveSurface,
            isAlreadyOffline: isAlreadyOffline,
            invisibleDuration: invisibleDuration,
            isIdle: isIdle
        )
    }

    @Test("idle requires no running process and no alternate screen")
    func idleRequiresNoProcessAndNoAltScreen() {
        #expect(TerminalOfflinePolicy.isIdle(hasRunningProcess: false, isAlternateScreen: false))
        #expect(!TerminalOfflinePolicy.isIdle(hasRunningProcess: true, isAlternateScreen: false))
        #expect(!TerminalOfflinePolicy.isIdle(hasRunningProcess: false, isAlternateScreen: true))
        #expect(!TerminalOfflinePolicy.isIdle(hasRunningProcess: true, isAlternateScreen: true))
    }

    @Test("a shell sitting at its prompt does not count as a running process")
    func shellAtPromptCountsAsIdle() {
        #expect(!hasRunningProcess(name: "zsh", arguments: ["-zsh"]))
        #expect(!hasRunningProcess(name: "-zsh", arguments: ["-zsh"]))
        #expect(!hasRunningProcess(name: "bash", arguments: ["bash", "-l"]))
        #expect(!hasRunningProcess(name: "bash", arguments: ["bash", "--posix"]))
        #expect(!hasRunningProcess(name: "fish", arguments: ["fish", "-i"]))
        #expect(!hasRunningProcess(name: "nu", arguments: ["nu", "--execute", "use ghostty *"]))
        #expect(!hasRunningProcess(name: "xonsh", arguments: ["xonsh"]))
    }

    @Test("a non-shell foreground process counts as running")
    func nonShellForegroundCountsAsRunning() {
        #expect(hasRunningProcess(name: "bun", arguments: ["bun", "run", "agent"]))
        #expect(hasRunningProcess(name: "node", arguments: ["node", "agent.js"]))
        #expect(hasRunningProcess(name: "ssh", arguments: ["ssh", "host"]))
        #expect(hasRunningProcess(name: "make", arguments: ["make", "test"]))
    }

    @Test("a shell executing a command or script counts as running")
    func executingShellCountsAsRunning() {
        #expect(hasRunningProcess(name: "bash", arguments: ["bash", "-c", "sleep 600"]))
        #expect(hasRunningProcess(name: "bash", arguments: ["bash", "--posix", "-c", "sleep 600"]))
        #expect(hasRunningProcess(name: "nu", arguments: ["nu", "--execute", "sleep 600"]))
        #expect(hasRunningProcess(
            name: "nu",
            arguments: ["nu", "--execute", "use ghostty *", "--command", "sleep 600"]
        ))
        #expect(hasRunningProcess(name: "zsh", arguments: ["zsh", "script.zsh"]))
        #expect(hasRunningProcess(name: "pwsh", arguments: ["pwsh", "-Command", "Start-Sleep 600"]))
        #expect(hasRunningProcess(name: "zsh", arguments: ["-zsh"], isShellCommandRunning: true))
    }

    @Test("an unknown foreground process never reports the pane as free")
    func unknownForegroundProcessFailsSafe() {
        #expect(hasRunningProcess(name: nil, arguments: nil))
        #expect(hasRunningProcess(name: "", arguments: []))
        #expect(hasRunningProcess(name: "zsh", arguments: nil))
        #expect(hasRunningProcess(name: "someunknownshell", arguments: ["someunknownshell"]))
    }

    @Test("a pane keeps awake only while on screen and focused")
    func keepsAwakeOnlyWhenOnScreenAndFocused() {
        #expect(TerminalOfflinePolicy.keepsAwake(isOnScreen: true, isFocused: true))
        #expect(!TerminalOfflinePolicy.keepsAwake(isOnScreen: true, isFocused: false))
        #expect(!TerminalOfflinePolicy.keepsAwake(isOnScreen: false, isFocused: true))
        #expect(!TerminalOfflinePolicy.keepsAwake(isOnScreen: false, isFocused: false))
    }

    @Test("takes an idle hidden surface offline once the idle threshold elapses")
    func takesIdleHiddenSurfaceOffline() {
        #expect(TerminalOfflinePolicy.shouldTakeOffline(
            candidate(invisibleDuration: 300),
            isEnabled: true,
            idleThreshold: 300
        ))
    }

    @Test("never offlines while disabled")
    func neverOfflinesWhileDisabled() {
        #expect(!TerminalOfflinePolicy.shouldTakeOffline(
            candidate(invisibleDuration: 9999),
            isEnabled: false,
            idleThreshold: 300
        ))
    }

    @Test("never offlines a visible surface")
    func neverOfflinesVisibleSurface() {
        #expect(!TerminalOfflinePolicy.shouldTakeOffline(
            candidate(invisibleDuration: nil),
            isEnabled: true,
            idleThreshold: 300
        ))
    }

    @Test("never offlines before the threshold elapses")
    func waitsForThreshold() {
        #expect(!TerminalOfflinePolicy.shouldTakeOffline(
            candidate(invisibleDuration: 120),
            isEnabled: true,
            idleThreshold: 300
        ))
    }

    @Test("never offlines a busy surface")
    func neverOfflinesBusySurface() {
        #expect(!TerminalOfflinePolicy.shouldTakeOffline(
            candidate(isIdle: false),
            isEnabled: true,
            idleThreshold: 300
        ))
    }

    @Test("never offlines without a live surface or when already offline")
    func skipsWhenNoSurfaceOrAlreadyOffline() {
        #expect(!TerminalOfflinePolicy.shouldTakeOffline(
            candidate(hasLiveSurface: false),
            isEnabled: true,
            idleThreshold: 300
        ))
        #expect(!TerminalOfflinePolicy.shouldTakeOffline(
            candidate(isAlreadyOffline: true),
            isEnabled: true,
            idleThreshold: 300
        ))
    }

    private func hasRunningProcess(
        name: String?,
        arguments: [String]?,
        isShellCommandRunning: Bool = false
    ) -> Bool {
        TerminalOfflinePolicy.hasRunningProcess(
            foregroundProcessName: name,
            foregroundProcessArguments: arguments,
            isShellCommandRunning: isShellCommandRunning
        )
    }
}
