import Foundation
import os
import Security

private let logger = Logger(subsystem: "app.muxy", category: "KeychainSSHHelper")

public enum KeychainSSHHelper {
    private static let serviceName = "com.muxy.ssh"

    public static func storePassword(
        _ password: String,
        host: String,
        user: String,
        port: UInt16 = 22,
        keyFingerprint: String? = nil
    ) {
        deletePassword(host: host, user: user, port: port, keyFingerprint: keyFingerprint)

        guard let passwordData = password.data(using: .utf8) else { return }

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: serviceName,
            kSecAttrAccount as String: account(host: host, user: user, port: port, keyFingerprint: keyFingerprint),
            kSecValueData as String: passwordData,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlocked,
        ]

        let status = SecItemAdd(query as CFDictionary, nil)
        if status != errSecSuccess {
            logger.error("Keychain store failed: \(status)")
        }
    }

    public static func getPassword(
        host: String,
        user: String,
        port: UInt16 = 22,
        keyFingerprint: String? = nil
    ) -> String? {
        if let exact = getPassword(account: account(host: host, user: user, port: port, keyFingerprint: keyFingerprint)) {
            return exact
        }

        if let fallback = getPassword(account: account(host: host, user: user, port: port)) {
            return fallback
        }

        if let legacy = getPassword(account: legacyAccount(host: host, user: user)) {
            return legacy
        }

        return nil
    }

    private static func getPassword(account: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: serviceName,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]

        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)

        guard status == errSecSuccess,
              let data = result as? Data,
              let password = String(data: data, encoding: .utf8)
        else {
            return nil
        }

        return password
    }

    public static func deletePassword(
        host: String,
        user: String,
        port: UInt16 = 22,
        keyFingerprint: String? = nil
    ) {
        deletePassword(account: account(host: host, user: user, port: port, keyFingerprint: keyFingerprint))
        deletePassword(account: account(host: host, user: user, port: port))
        deletePassword(account: legacyAccount(host: host, user: user))
    }

    private static func deletePassword(account: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: serviceName,
            kSecAttrAccount as String: account,
        ]

        SecItemDelete(query as CFDictionary)
    }

    private static func account(
        host: String,
        user: String,
        port: UInt16,
        keyFingerprint: String? = nil
    ) -> String {
        if let keyFingerprint, !keyFingerprint.isEmpty {
            return "\(user)@\(host):\(port)#\(keyFingerprint)"
        }
        return "\(user)@\(host):\(port)"
    }

    private static func legacyAccount(host: String, user: String) -> String {
        "\(user)@\(host)"
    }
}
