import Foundation
import Testing

@testable import Muxy

@Suite("Update session restoration")
struct UpdateSessionRestorationTests {
    @Test("matching target build enables restoration once")
    func matchingTargetBuild() throws {
        let defaults = try makeDefaults()
        UpdateSessionRestoration.mark(
            targetBuild: "200",
            currentBuild: "100",
            defaults: defaults
        )
        UpdateSessionRestoration.armForTermination(defaults: defaults)
        UpdateSessionRestoration.mark(
            targetBuild: "200",
            currentBuild: "100",
            defaults: defaults
        )

        #expect(UpdateSessionRestoration.consumeEligibility(
            currentBuild: "200",
            defaults: defaults
        ) == true)
        #expect(UpdateSessionRestoration.consumeEligibility(
            currentBuild: "200",
            defaults: defaults
        ) == false)
    }

    @Test("reopening the source build disarms a failed installation")
    func sourceBuildDisarmsRestoration() throws {
        let defaults = try makeDefaults()
        UpdateSessionRestoration.mark(
            targetBuild: "200",
            currentBuild: "100",
            defaults: defaults
        )
        UpdateSessionRestoration.armForTermination(defaults: defaults)

        #expect(UpdateSessionRestoration.consumeEligibility(
            currentBuild: "100",
            defaults: defaults
        ) == false)
        #expect(UpdateSessionRestoration.consumeEligibility(
            currentBuild: "200",
            defaults: defaults
        ) == false)
    }

    @Test("scheduled update without accepted termination does not restore")
    func unarmedUpdate() throws {
        let defaults = try makeDefaults()
        UpdateSessionRestoration.mark(
            targetBuild: "200",
            currentBuild: "100",
            defaults: defaults
        )

        #expect(UpdateSessionRestoration.consumeEligibility(
            currentBuild: "200",
            defaults: defaults
        ) == false)
    }

    @Test("unrelated build discards stale restoration")
    func unrelatedBuild() throws {
        let defaults = try makeDefaults()
        UpdateSessionRestoration.mark(
            targetBuild: "200",
            currentBuild: "100",
            defaults: defaults
        )

        #expect(UpdateSessionRestoration.consumeEligibility(
            currentBuild: "300",
            defaults: defaults
        ) == false)
        #expect(defaults.object(forKey: UpdateSessionRestoration.storageKey) == nil)
    }

    @Test("malformed restoration state is deleted instead of armed")
    func malformedStateCannotBeArmed() throws {
        let defaults = try makeDefaults()
        defaults.set(
            ["sourceBuild": "100", "targetBuild": "", "armed": false],
            forKey: UpdateSessionRestoration.storageKey
        )

        UpdateSessionRestoration.armForTermination(defaults: defaults)

        #expect(defaults.object(forKey: UpdateSessionRestoration.storageKey) == nil)
        #expect(!UpdateSessionRestoration.consumeEligibility(currentBuild: "200", defaults: defaults))
    }

    @Test("invalid update metadata clears an existing armed marker")
    func invalidMarkClearsExistingState() throws {
        let defaults = try makeDefaults()
        UpdateSessionRestoration.mark(targetBuild: "200", currentBuild: "100", defaults: defaults)
        UpdateSessionRestoration.armForTermination(defaults: defaults)

        UpdateSessionRestoration.mark(targetBuild: "", currentBuild: "100", defaults: defaults)

        #expect(defaults.object(forKey: UpdateSessionRestoration.storageKey) == nil)
        #expect(!UpdateSessionRestoration.consumeEligibility(currentBuild: "200", defaults: defaults))
    }

    @Test("aborted updates discard their restoration marker")
    func abortedUpdateInvalidatesState() throws {
        let defaults = try makeDefaults()
        UpdateSessionRestoration.mark(targetBuild: "200", currentBuild: "100", defaults: defaults)

        UpdateSessionRestoration.invalidate(defaults: defaults)
        UpdateSessionRestoration.armForTermination(defaults: defaults)

        #expect(!UpdateSessionRestoration.consumeEligibility(currentBuild: "200", defaults: defaults))
    }

    private func makeDefaults() throws -> UserDefaults {
        let suiteName = "UpdateSessionRestorationTests-\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defaults.removePersistentDomain(forName: suiteName)
        return defaults
    }
}
