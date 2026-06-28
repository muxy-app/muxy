import AppKit
import Foundation

struct InstalledBrowser: Identifiable, Hashable {
    let id: String
    let name: String
    let appURL: URL

    var icon: NSImage {
        NSWorkspace.shared.icon(forFile: appURL.path)
    }
}

enum InstalledBrowsers {
    private static let probeURL = URL(string: "https://example.com") ?? URL(filePath: "/")

    static func all() -> [InstalledBrowser] {
        let urls = NSWorkspace.shared.urlsForApplications(toOpen: probeURL)
        let browsers = urls.compactMap(browser(from:))
        var seen = Set<String>()
        return browsers
            .filter { seen.insert($0.id).inserted }
            .sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    static func defaultBrowser() -> InstalledBrowser? {
        guard let url = NSWorkspace.shared.urlForApplication(toOpen: probeURL) else { return nil }
        return browser(from: url)
    }

    static func open(_ url: URL, in browser: InstalledBrowser) {
        let configuration = NSWorkspace.OpenConfiguration()
        NSWorkspace.shared.open([url], withApplicationAt: browser.appURL, configuration: configuration)
    }

    private static func browser(from appURL: URL) -> InstalledBrowser? {
        guard let bundle = Bundle(url: appURL),
              let bundleID = bundle.bundleIdentifier
        else { return nil }
        let name = (bundle.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String)
            ?? (bundle.object(forInfoDictionaryKey: "CFBundleName") as? String)
            ?? appURL.deletingPathExtension().lastPathComponent
        return InstalledBrowser(id: bundleID, name: name, appURL: appURL)
    }
}
