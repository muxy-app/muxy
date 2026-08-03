import Foundation
import Testing

@Suite("Localization catalogs")
struct LocalizationCatalogTests {
    @Test("every language maps exactly the English keys")
    func everyLanguageMapsEnglishKeys() throws {
        let localizationDirectory = RepositoryRoot.find()
            .appendingPathComponent("Muxy/Resources/Localization")
        let languageDirectories = try FileManager.default.contentsOfDirectory(
            at: localizationDirectory,
            includingPropertiesForKeys: [.isDirectoryKey],
            options: [.skipsHiddenFiles]
        )
            .filter { url in
                url.pathExtension == "lproj"
                    && (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true
            }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
        let englishCatalogURL = localizationDirectory
            .appendingPathComponent("en.lproj/Localizable.strings")
        let englishKeys = Set(try catalog(at: englishCatalogURL).keys)

        try #require(!languageDirectories.isEmpty)
        for languageDirectory in languageDirectories {
            let catalogURL = languageDirectory.appendingPathComponent("Localizable.strings")
            let language = languageDirectory.deletingPathExtension().lastPathComponent
            let localizedKeys = Set(try catalog(at: catalogURL).keys)
            let missingKeys = englishKeys.subtracting(localizedKeys).sorted()
            let unexpectedKeys = localizedKeys.subtracting(englishKeys).sorted()

            #expect(missingKeys.isEmpty, "\(language) is missing keys: \(missingKeys)")
            #expect(unexpectedKeys.isEmpty, "\(language) has unexpected keys: \(unexpectedKeys)")
        }
    }

    private func catalog(at url: URL) throws -> [String: String] {
        let data = try Data(contentsOf: url)
        let propertyList = try PropertyListSerialization.propertyList(from: data, format: nil)
        return try #require(propertyList as? [String: String])
    }
}
