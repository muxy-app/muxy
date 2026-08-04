import Foundation
import Testing

@testable import Muxy

@Suite("TipsStore")
@MainActor
struct TipsStoreTests {
    @Test("catalog decodes description-only tips")
    func catalogDecodesDescriptions() throws {
        let tips = try TipCatalog.decode(Data(#"[{"description":" First tip "},{"description":"Second tip"}]"#.utf8))

        #expect(tips == [try MuxyTip(description: "First tip"), try MuxyTip(description: "Second tip")])
    }

    @Test("catalog rejects empty collections")
    func catalogRejectsEmptyCollection() {
        #expect(throws: DecodingError.self) {
            try TipCatalog.decode(Data("[]".utf8))
        }
    }

    @Test("catalog rejects blank descriptions")
    func catalogRejectsBlankDescription() {
        #expect(throws: DecodingError.self) {
            try TipCatalog.decode(Data(#"[{"description":"   "}]"#.utf8))
        }
    }

    @Test("catalog rejects fields other than description")
    func catalogRejectsUnknownFields() {
        #expect(throws: DecodingError.self) {
            try TipCatalog.decode(Data(#"[{"description":"Tip","name":"Extra"}]"#.utf8))
        }
    }

    @Test("bundled catalog contains only description fields")
    func bundledCatalogSchema() throws {
        let url = RepositoryRoot.find().appendingPathComponent("Muxy/Resources/tips.json")
        let data = try Data(contentsOf: url)
        let objects = try #require(JSONSerialization.jsonObject(with: data) as? [[String: Any]])

        #expect(!objects.isEmpty)
        #expect(objects.allSatisfy { Set($0.keys) == ["description"] })
        #expect(try TipCatalog.decode(data).count == objects.count)
    }

    @Test("bundled descriptions exist in the English localization template")
    func bundledDescriptionsAreLocalizable() throws {
        let root = RepositoryRoot.find()
        let tips = try TipCatalog.decode(Data(contentsOf: root.appendingPathComponent("Muxy/Resources/tips.json")))
        let localizationURL = root.appendingPathComponent(
            "Muxy/Resources/Localization/en.lproj/Localizable.strings"
        )
        let localizationData = try Data(contentsOf: localizationURL)
        let localization = try #require(
            PropertyListSerialization.propertyList(from: localizationData, format: nil) as? [String: String]
        )
        let missingDescriptions = tips.map(\.description).filter { localization[$0] == nil }

        #expect(missingDescriptions.isEmpty, "Missing tip localization keys: \(missingDescriptions)")
    }

    @Test("bundled documentation links map to local documentation")
    func bundledDocumentationLinksResolveLocally() throws {
        let root = RepositoryRoot.find()
        let tips = try TipCatalog.decode(Data(contentsOf: root.appendingPathComponent("Muxy/Resources/tips.json")))
        let linkedDocumentationURLs = tips.flatMap { documentationURLs(in: $0.description) }
        let invalidURLs = linkedDocumentationURLs.filter { documentationPath(for: $0) == nil }
        let linkedDocumentationPaths = linkedDocumentationURLs.compactMap(documentationPath)
        let missingPaths = linkedDocumentationPaths.filter { path in
            !FileManager.default.fileExists(
                atPath: root.appendingPathComponent("docs/\(path).md").path
            )
        }

        #expect(!linkedDocumentationURLs.isEmpty)
        #expect(invalidURLs.isEmpty, "Invalid local documentation links: \(invalidURLs)")
        #expect(missingPaths.isEmpty, "Missing local documentation for tip links: \(missingPaths)")
    }

    @Test("documentation links reject traversal outside docs", arguments: [
        "https://muxy.app/docs/../README",
        "https://muxy.app/docs/%2e%2e/README",
        "https://muxy.app/docs/%2F..%2FREADME",
        "https://muxy.app/docs/%5C..%5CREADME",
    ])
    func documentationLinksRejectTraversal(rawURL: String) throws {
        let url = try #require(URL(string: rawURL))

        #expect(documentationPath(for: url) == nil)
    }

    @Test("module resource bundle contains the tips catalog")
    func moduleBundleContainsCatalog() {
        #expect(!TipCatalog.load(bundle: .module).isEmpty)
    }

    @Test("starting tip is selected once during initialization")
    func startingTipIsStable() throws {
        var selections = 0
        let store = TipsStore(tips: try tips()) { count in
            selections += 1
            #expect(count == 3)
            return 1
        }

        #expect(store.currentTip?.description == "Second")
        #expect(store.currentTip?.description == "Second")
        #expect(selections == 1)
    }

    @Test("navigation wraps in catalog order")
    func navigationWraps() throws {
        let store = TipsStore(tips: try tips()) { _ in 0 }

        store.showPrevious()
        #expect(store.currentTip?.description == "Third")
        #expect(store.position == 3)

        store.showNext()
        #expect(store.currentTip?.description == "First")
        #expect(store.position == 1)
    }

    @Test("empty stores ignore navigation")
    func emptyStoreIgnoresNavigation() {
        let store = TipsStore(tips: []) { _ in
            Issue.record("Empty stores must not request a starting index")
            return 0
        }

        store.showNext()
        store.showPrevious()

        #expect(store.currentTip == nil)
        #expect(store.position == 0)
    }

    private func tips() throws -> [MuxyTip] {
        [
            try MuxyTip(description: "First"),
            try MuxyTip(description: "Second"),
            try MuxyTip(description: "Third"),
        ]
    }

    private func documentationURLs(in description: String) -> [URL] {
        let prefix = "https://muxy.app/docs/"
        var urls: [URL] = []
        var searchStart = description.startIndex
        while let range = description.range(of: prefix, range: searchStart ..< description.endIndex) {
            let suffix = description[range.upperBound...]
            let target = suffix.prefix { $0 != ")" && !$0.isWhitespace }
            if let url = URL(string: prefix + String(target)) {
                urls.append(url)
            }
            searchStart = description.index(range.upperBound, offsetBy: target.count)
        }
        return urls
    }

    private func documentationPath(for url: URL) -> String? {
        guard url.scheme == "https", url.host == "muxy.app" else { return nil }
        let components = url.pathComponents
        guard components.count > 2, components[0] == "/", components[1] == "docs" else { return nil }
        let relativeComponents = components.dropFirst(2)
        guard relativeComponents.allSatisfy({ component in
            component != "." && component != ".." && !component.isEmpty
                && !component.contains("/") && !component.contains("\\")
        }) else { return nil }
        return relativeComponents.joined(separator: "/")
    }
}
