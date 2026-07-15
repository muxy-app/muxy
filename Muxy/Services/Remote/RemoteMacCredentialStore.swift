import Foundation
import Security

struct RemoteMacCredentials: Codable, Equatable {
    let deviceID: UUID
    let token: String
    let endpointScope: String?

    init(deviceID: UUID, token: String, endpointScope: String? = nil) {
        self.deviceID = deviceID
        self.token = token
        self.endpointScope = endpointScope
    }
}

protocol RemoteMacCredentialStoring: Sendable {
    func loadOrCreate(for deviceID: UUID, endpointScope: String) throws -> RemoteMacCredentials
    func delete(for deviceID: UUID, endpointScope: String?) throws
}

extension RemoteMacCredentialStoring {
    func delete(for deviceID: UUID) throws {
        try delete(for: deviceID, endpointScope: nil)
    }
}

enum RemoteMacCredentialStoreError: LocalizedError {
    case randomGenerationFailed(OSStatus)
    case keychainReadFailed(OSStatus)
    case keychainWriteFailed(OSStatus)
    case invalidStoredCredentials

    var errorDescription: String? {
        switch self {
        case let .randomGenerationFailed(status):
            "Could not generate remote access credentials (\(status))."
        case let .keychainReadFailed(status):
            "Could not read remote access credentials (\(status))."
        case let .keychainWriteFailed(status):
            "Could not save remote access credentials (\(status))."
        case .invalidStoredCredentials:
            "The stored remote access credentials are invalid."
        }
    }
}

final class KeychainRemoteMacCredentialStore: RemoteMacCredentialStoring, @unchecked Sendable {
    private let service: String

    init(service: String = "app.muxy.remote-mac") {
        self.service = service
    }

    func loadOrCreate(for deviceID: UUID, endpointScope: String) throws -> RemoteMacCredentials {
        if let credentials = try read(for: deviceID, endpointScope: endpointScope),
           credentials.endpointScope == endpointScope
        {
            return credentials
        }
        try delete(for: deviceID, endpointScope: endpointScope)
        let credentials = try RemoteMacCredentials(
            deviceID: UUID(),
            token: generateToken(),
            endpointScope: endpointScope
        )
        try save(credentials, for: deviceID)
        return credentials
    }

    func delete(for deviceID: UUID, endpointScope: String?) throws {
        let status = SecItemDelete(query(for: deviceID, endpointScope: endpointScope) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw RemoteMacCredentialStoreError.keychainWriteFailed(status)
        }
    }

    private func read(for deviceID: UUID, endpointScope: String) throws -> RemoteMacCredentials? {
        var query = query(for: deviceID, endpointScope: endpointScope)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess else {
            throw RemoteMacCredentialStoreError.keychainReadFailed(status)
        }
        guard let data = item as? Data,
              let credentials = try? JSONDecoder().decode(RemoteMacCredentials.self, from: data),
              !credentials.token.isEmpty
        else {
            throw RemoteMacCredentialStoreError.invalidStoredCredentials
        }
        return credentials
    }

    private func save(_ credentials: RemoteMacCredentials, for deviceID: UUID) throws {
        let data = try JSONEncoder().encode(credentials)
        var attributes = query(for: deviceID, endpointScope: credentials.endpointScope)
        attributes[kSecValueData as String] = data
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(attributes as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw RemoteMacCredentialStoreError.keychainWriteFailed(status)
        }
    }

    private func generateToken() throws -> String {
        var bytes = [UInt8](repeating: 0, count: 32)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else {
            throw RemoteMacCredentialStoreError.randomGenerationFailed(status)
        }
        return Data(bytes).base64EncodedString()
    }

    private func query(for deviceID: UUID, endpointScope: String?) -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "\(service).\(deviceID.uuidString)",
        ]
        if let endpointScope {
            query[kSecAttrAccount as String] = endpointScope
        }
        return query
    }
}

final class InMemoryRemoteMacCredentialStore: RemoteMacCredentialStoring, @unchecked Sendable {
    private struct CredentialKey: Hashable {
        let deviceID: UUID
        let endpointScope: String
    }

    private let lock = NSLock()
    private var values: [CredentialKey: RemoteMacCredentials]

    init(values: [UUID: RemoteMacCredentials] = [:]) {
        self.values = Dictionary(uniqueKeysWithValues: values.compactMap { deviceID, credentials in
            guard let endpointScope = credentials.endpointScope else { return nil }
            return (CredentialKey(deviceID: deviceID, endpointScope: endpointScope), credentials)
        })
    }

    func loadOrCreate(for deviceID: UUID, endpointScope: String) throws -> RemoteMacCredentials {
        lock.lock()
        defer { lock.unlock() }
        let key = CredentialKey(deviceID: deviceID, endpointScope: endpointScope)
        if let credentials = values[key] { return credentials }
        let credentials = RemoteMacCredentials(
            deviceID: UUID(),
            token: UUID().uuidString,
            endpointScope: endpointScope
        )
        values[key] = credentials
        return credentials
    }

    func delete(for deviceID: UUID, endpointScope: String?) throws {
        lock.lock()
        if let endpointScope {
            values.removeValue(forKey: CredentialKey(deviceID: deviceID, endpointScope: endpointScope))
        } else {
            values = values.filter { $0.key.deviceID != deviceID }
        }
        lock.unlock()
    }
}
