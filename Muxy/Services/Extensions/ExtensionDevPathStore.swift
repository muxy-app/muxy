import Foundation

enum ExtensionDevPathStore {
    private static let key = "muxy.ext.devPaths"

    static func paths(defaults: UserDefaults = .standard) -> [String] {
        defaults.stringArray(forKey: key) ?? []
    }

    static func add(_ path: String, defaults: UserDefaults = .standard) {
        let normalized = normalize(path)
        guard !normalized.isEmpty else { return }
        var current = paths(defaults: defaults)
        guard !current.contains(normalized) else { return }
        current.append(normalized)
        defaults.set(current, forKey: key)
    }

    static func remove(_ path: String, defaults: UserDefaults = .standard) {
        let normalized = normalize(path)
        let current = paths(defaults: defaults).filter { $0 != normalized }
        defaults.set(current, forKey: key)
    }

    private static func normalize(_ path: String) -> String {
        URL(fileURLWithPath: path).standardizedFileURL.path
    }
}
