import Testing

@testable import Muxy

@Suite("SleepingTabPlaceholderPolicy")
struct SleepingTabPlaceholderPolicyTests {
    @Test("presents when a visible local pane is offline")
    func presentsWhenVisibleOfflineLocal() {
        #expect(SleepingTabPlaceholderPolicy.shouldPresent(
            isVisible: true,
            isOffline: true,
            isRemotelyOwned: false
        ))
    }

    @Test("hides while the pane is not visible")
    func hidesWhenNotVisible() {
        #expect(!SleepingTabPlaceholderPolicy.shouldPresent(
            isVisible: false,
            isOffline: true,
            isRemotelyOwned: false
        ))
    }

    @Test("hides while the pane is online")
    func hidesWhenOnline() {
        #expect(!SleepingTabPlaceholderPolicy.shouldPresent(
            isVisible: true,
            isOffline: false,
            isRemotelyOwned: false
        ))
    }

    @Test("hides while the pane is owned by a remote device")
    func hidesWhenRemotelyOwned() {
        #expect(!SleepingTabPlaceholderPolicy.shouldPresent(
            isVisible: true,
            isOffline: true,
            isRemotelyOwned: true
        ))
    }
}
