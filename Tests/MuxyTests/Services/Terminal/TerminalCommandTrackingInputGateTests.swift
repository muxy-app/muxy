import Testing

@testable import Muxy

@Suite("TerminalCommandTrackingInputGate")
struct TerminalCommandTrackingInputGateTests {
    @Test("Allows shell input on primary screen")
    func allowsShellInputOnPrimaryScreen() {
        let context = TerminalCommandTrackingInputContext(altScreen: false, foregroundProcessName: "zsh")
        #expect(TerminalCommandTrackingInputGate.shouldRecordInput(context))
    }

    @Test("Blocks shell input on alternate screen")
    func blocksShellInputOnAlternateScreen() {
        let context = TerminalCommandTrackingInputContext(altScreen: true, foregroundProcessName: "zsh")
        #expect(!TerminalCommandTrackingInputGate.shouldRecordInput(context))
    }

    @Test("Blocks non-shell foreground process")
    func blocksNonShellForegroundProcess() {
        let context = TerminalCommandTrackingInputContext(altScreen: false, foregroundProcessName: "nvim")
        #expect(!TerminalCommandTrackingInputGate.shouldRecordInput(context))
    }

    @Test("Allows missing foreground process as fallback")
    func allowsMissingForegroundProcessAsFallback() {
        let context = TerminalCommandTrackingInputContext(altScreen: false, foregroundProcessName: nil)
        #expect(TerminalCommandTrackingInputGate.shouldRecordInput(context))
    }

    @Test("Matches shell names case-insensitively")
    func matchesShellNamesCaseInsensitively() {
        let context = TerminalCommandTrackingInputContext(altScreen: false, foregroundProcessName: "ZSH")
        #expect(TerminalCommandTrackingInputGate.shouldRecordInput(context))
    }

    @Test("Allows terminal multiplexers on primary screen")
    func allowsTerminalMultiplexersOnPrimaryScreen() {
        let context = TerminalCommandTrackingInputContext(altScreen: false, foregroundProcessName: "tmux")
        #expect(TerminalCommandTrackingInputGate.shouldRecordInput(context))
    }
}
