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
}
