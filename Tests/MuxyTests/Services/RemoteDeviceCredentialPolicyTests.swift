import Foundation
import Testing

@testable import Muxy

@Suite("RemoteDeviceCredentialPolicy")
struct RemoteDeviceCredentialPolicyTests {
    @Test("new password overrides existing password")
    func newPasswordOverridesExisting() {
        #expect(
            RemoteDeviceCredentialPolicy.passwordToPersist(
                typedPassword: " new-secret ",
                existingPassword: "old-secret"
            ) == "new-secret"
        )
    }

    @Test("existing password is reused when edit leaves password blank")
    func existingPasswordIsReused() {
        #expect(
            RemoteDeviceCredentialPolicy.passwordToPersist(
                typedPassword: "   ",
                existingPassword: "old-secret"
            ) == "old-secret"
        )
    }

    @Test("password auth cannot save without new or existing password")
    func passwordAuthRequiresCredential() {
        #expect(
            !RemoteDeviceCredentialPolicy.canSavePasswordAuthentication(
                typedPassword: "   ",
                existingPassword: nil
            )
        )
    }
}
