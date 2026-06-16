import Foundation
import Testing

@testable import Muxy

@Suite("LoginShellPath")
struct LoginShellPathTests {
    @Test("hydrate waits for resolved login shell PATH")
    func hydrateWaitsForResolvedLoginShellPath() async {
        let path = LoginShellPath()

        await path.hydrate {
            "/tmp/custom-bin:/usr/bin"
        }

        #expect(path.value == "/tmp/custom-bin:/usr/bin")
    }

    @Test("hydrate keeps default PATH when lookup fails")
    func hydrateKeepsDefaultPathWhenLookupFails() async {
        let path = LoginShellPath()

        await path.hydrate {
            nil
        }

        #expect(path.value == LoginShellPath.defaultPath)
    }
}
