import Foundation

enum ExtensionManifestFixture {
    static func packageJSON(fromFlatManifest manifest: String) -> Data {
        let object = (try? JSONSerialization.jsonObject(with: Data(manifest.utf8))) as? [String: Any] ?? [:]

        var muxy = object
        let name = muxy.removeValue(forKey: "name") as? String ?? ""
        let version = muxy.removeValue(forKey: "version") as? String ?? "0.0.0"
        let enabled = muxy.removeValue(forKey: "enabled")

        var package: [String: Any] = [
            "name": name,
            "version": version,
            "private": true,
            "scripts": ["build": "vite build"],
            "muxy": muxy,
        ]
        if let enabled {
            package["enabled"] = enabled
        }

        return (try? JSONSerialization.data(withJSONObject: package, options: [.sortedKeys])) ?? Data()
    }

    @discardableResult
    static func write(flatManifest manifest: String, to directory: URL) throws -> URL {
        let url = directory.appendingPathComponent("package.json")
        try packageJSON(fromFlatManifest: manifest).write(to: url)
        return url
    }
}
