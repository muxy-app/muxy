import Foundation

enum RemoteDeviceCredentialPolicy {
    static func canSavePasswordAuthentication(
        typedPassword: String,
        existingPassword: String?
    ) -> Bool {
        passwordToPersist(typedPassword: typedPassword, existingPassword: existingPassword) != nil
    }

    static func passwordToPersist(
        typedPassword: String,
        existingPassword: String?
    ) -> String? {
        let trimmedPassword = typedPassword.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedPassword.isEmpty {
            return trimmedPassword
        }
        guard let existingPassword, !existingPassword.isEmpty else { return nil }
        return existingPassword
    }
}
