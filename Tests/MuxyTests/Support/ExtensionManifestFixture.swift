import Foundation

/// Test helpers for the `package.json` manifest format.
///
/// Extensions ship their manifest as the `muxy` object of an npm `package.json`
/// (`name`/`version` are the top-level npm fields). Tests historically wrote a
/// flat `manifest.json`-style JSON string; these helpers wrap such a string into
/// the `package.json` envelope so existing fixtures keep working unchanged.
enum ExtensionManifestFixture {
    /// Wraps a flat manifest JSON string (the old `manifest.json` shape, with
    /// `name`/`version` plus manifest fields at the top level) into a
    /// `package.json`: `name`/`version` stay top-level, a `build` script is
    /// added, and every other field moves under `muxy`.
    static func packageJSON(fromFlatManifest manifest: String) -> Data {
        let object = (try? JSONSerialization.jsonObject(with: Data(manifest.utf8))) as? [String: Any] ?? [:]

        var muxy = object
        let name = muxy.removeValue(forKey: "name") as? String ?? ""
        let version = muxy.removeValue(forKey: "version") as? String ?? "0.0.0"
        // A stray top-level `enabled` flag (legacy) is preserved at the top
        // level so the loader's legacy-migration path still sees it.
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

    /// Writes a `package.json` built from a flat manifest string into `directory`.
    @discardableResult
    static func write(flatManifest manifest: String, to directory: URL) throws -> URL {
        let url = directory.appendingPathComponent("package.json")
        try packageJSON(fromFlatManifest: manifest).write(to: url)
        return url
    }
}
