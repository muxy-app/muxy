import Foundation

final class ChromeImporter: BrowserImporter, Sendable {
    let source: BrowserImportSource = .chrome

    private let keychainService = "Chrome Safe Storage"
    private let keychainAccount = "Chrome"
    private let chromeEpochOffset: TimeInterval = 11_644_473_600

    private var rootDirectory: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Google/Chrome", isDirectory: true)
    }

    func isInstalled() -> Bool {
        FileManager.default.fileExists(atPath: rootDirectory.path)
    }

    func availableProfiles() throws -> [ImportableProfile] {
        guard isInstalled() else { throw BrowserImportError.sourceNotInstalled }
        let names = profileDisplayNames()
        let contents = (try? FileManager.default.contentsOfDirectory(
            at: rootDirectory,
            includingPropertiesForKeys: [.isDirectoryKey]
        )) ?? []

        let profiles = contents.compactMap { url -> ImportableProfile? in
            let dirName = url.lastPathComponent
            guard dirName == "Default" || dirName.hasPrefix("Profile ") else { return nil }
            guard FileManager.default.fileExists(atPath: url.appendingPathComponent("Cookies").path)
            else { return nil }
            return ImportableProfile(id: dirName, name: names[dirName] ?? dirName, directory: url)
        }
        return profiles.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
    }

    func importCookies(from profile: ImportableProfile) throws -> [HTTPCookie] {
        guard let password = ChromeSafeStorageKeychain.password(
            service: keychainService,
            account: keychainAccount
        )
        else {
            throw BrowserImportError.keyUnavailable
        }
        guard let key = ChromeCookieDecryptor.deriveKey(fromSafeStoragePassword: password) else {
            throw BrowserImportError.keyUnavailable
        }

        let rows = try ChromeCookieDatabase.readRows(at: profile.directory.appendingPathComponent("Cookies"))
        return rows.compactMap { cookie(from: $0, key: key) }
    }

    private func cookie(from row: ChromeCookieRow, key: Data) -> HTTPCookie? {
        guard !row.encryptedValue.isEmpty,
              let value = ChromeCookieDecryptor.decrypt(encryptedValue: row.encryptedValue, host: row.host, key: key)
        else { return nil }

        var properties: [HTTPCookiePropertyKey: Any] = [
            .name: row.name,
            .value: value,
            .domain: row.host,
            .path: row.path.isEmpty ? "/" : row.path,
        ]
        if row.isSecure { properties[.secure] = "TRUE" }
        if row.isHTTPOnly { properties[HTTPCookiePropertyKey("HttpOnly")] = "TRUE" }
        if let expiry = expiryDate(from: row.expiresUTC) { properties[.expires] = expiry }

        return HTTPCookie(properties: properties)
    }

    private func expiryDate(from chromeExpiresUTC: Int64) -> Date? {
        guard chromeExpiresUTC > 0 else { return nil }
        let secondsSince1601 = TimeInterval(chromeExpiresUTC) / 1_000_000
        return Date(timeIntervalSince1970: secondsSince1601 - chromeEpochOffset)
    }

    private func profileDisplayNames() -> [String: String] {
        let localStateURL = rootDirectory.appendingPathComponent("Local State")
        guard let data = try? Data(contentsOf: localStateURL),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let profile = json["profile"] as? [String: Any],
              let infoCache = profile["info_cache"] as? [String: Any]
        else { return [:] }

        var names: [String: String] = [:]
        for (dirName, info) in infoCache {
            if let info = info as? [String: Any], let name = info["name"] as? String {
                names[dirName] = name
            }
        }
        return names
    }
}
