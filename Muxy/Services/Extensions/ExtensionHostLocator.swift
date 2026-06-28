import Foundation

enum ExtensionHostLocator {
    static let binaryName = "MuxyExtensionHost"

    static func hostURL() -> URL? {
        guard let executableURL = Bundle.main.executableURL else { return nil }
        let sibling = executableURL.deletingLastPathComponent().appendingPathComponent(binaryName)
        guard FileManager.default.isExecutableFile(atPath: sibling.path) else { return nil }
        return sibling
    }
}
