import Testing

@testable import Muxy

@Suite("AppRelaunch")
@MainActor
struct AppRelaunchTests {
    @Test("relaunch suppresses termination user-state persistence")
    func relaunchSuppressesTerminationUserStatePersistence() {
        AppRelaunch.resetForTesting()
        defer { AppRelaunch.resetForTesting() }

        let delegate = AppDelegate()
        var didPersist = false
        delegate.onTerminate = {
            didPersist = true
        }

        AppRelaunch.prepareForRelaunch()
        delegate.persistUserStateForTermination()

        #expect(!didPersist)
    }
}
