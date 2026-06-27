import Foundation

extension Bundle {
    static let appResources: Bundle = {
        let bundleName = "Muxy_Muxy.bundle"

        let candidates: [URL?] = [
            Bundle.main.resourceURL?.appendingPathComponent(bundleName),
            Bundle.main.bundleURL.appendingPathComponent(bundleName),
            Bundle.main.bundleURL.appendingPathComponent("Contents/Resources/\(bundleName)"),
        ]

        for case let url? in candidates {
            if let bundle = Bundle(url: url) {
                return bundle
            }
        }

        return Bundle.main
    }()

    static var providerIconsURL: URL? {
        guard let resourceURL = appResources.resourceURL else { return nil }
        let candidate = resourceURL.appendingPathComponent("ProviderIcons")
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: candidate.path, isDirectory: &isDirectory),
              isDirectory.boolValue
        else { return nil }
        return candidate
    }
}
