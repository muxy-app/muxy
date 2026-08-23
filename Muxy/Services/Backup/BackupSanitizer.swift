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

    static func sanitizedWorkspaces(at url: URL) throws -> Data {
        let object = try JSONSerialization.jsonObject(with: Data(contentsOf: url))
        let sanitizedObject: Any = if let workspaces = object as? [[String: Any]] {
            workspaces.map { workspace -> [String: Any] in
                var sanitized = workspace
                guard let root = workspace["root"] as? [String: Any] else { return workspace }
                sanitized["root"] = sanitizingSplitNode(root)
                return sanitized
            }
        } else {
            object
        }
        return try JSONSerialization.data(
            withJSONObject: sanitizedObject,
            options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        )
    }

    private static func sanitizingSplitNode(_ node: [String: Any]) -> [String: Any] {
        guard let type = node["type"] as? String else { return node }
        var sanitized = node
        if type == "tabArea", var area = node["tabArea"] as? [String: Any] {
            if let tabs = area["tabs"] as? [[String: Any]] {
                area["tabs"] = tabs.map(sanitizingTerminalTab)
            }
            sanitized["tabArea"] = area
        }
        if type == "split", var split = node["split"] as? [String: Any] {
            if let first = split["first"] as? [String: Any] {
                split["first"] = sanitizingSplitNode(first)
            }
            if let second = split["second"] as? [String: Any] {
                split["second"] = sanitizingSplitNode(second)
            }
            sanitized["split"] = split
        }
        return sanitized
    }

    private static func sanitizingTerminalTab(_ tab: [String: Any]) -> [String: Any] {
        var sanitized = tab
        if var destination = tab["paneRemoteTmuxDestination"] as? [String: Any] {
            destination["environment"] = SSHEnvironmentVariables.default
            sanitized["paneRemoteTmuxDestination"] = destination
        }
        return sanitized
    }
}
