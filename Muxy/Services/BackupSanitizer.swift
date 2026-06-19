import Foundation

enum BackupSanitizer {
    static func sanitizedRemoteDevices(at url: URL) throws -> Data {
        let devices = try JSONDecoder().decode([RemoteDevice].self, from: Data(contentsOf: url))
        let sanitized = devices.map { device -> RemoteDevice in
            var copy = device
            copy.ssh.environment = SSHEnvironmentVariables.default
            return copy
        }
        return try JSONEncoder().encode(sanitized)
    }

    static func sanitizedSettings(at url: URL) throws -> Data {
        let object = try JSONSerialization.jsonObject(with: Data(contentsOf: url))
        guard var dictionary = object as? [String: Any] else { return try Data(contentsOf: url) }
        dictionary["mobile.approvedDevices"] = []
        return try JSONSerialization.data(
            withJSONObject: dictionary,
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        )
    }
}
