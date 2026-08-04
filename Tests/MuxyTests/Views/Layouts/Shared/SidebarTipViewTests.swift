import Foundation
import Testing

@testable import Muxy

@Suite("SidebarTipView")
@MainActor
struct SidebarTipViewTests {
    @Test("tip descriptions are localized before Markdown parsing")
    func descriptionLocalizationPreservesMarkdownLinks() throws {
        let description = "Read [tips](https://muxy.app/docs)."
        let fixture = try LocalizationTestSupport.makeService(
            translations: #"""
            "Read [tips](https://muxy.app/docs)." = "Lies [Tipps](https://muxy.app/docs).";
            """#
        )
        defer { try? FileManager.default.removeItem(at: fixture.root) }

        let attributedDescription = TipDescriptionPresentation.attributedDescription(
            description,
            localization: fixture.service
        )

        #expect(String(attributedDescription.characters) == "Lies Tipps.")
        #expect(attributedDescription.runs.contains { $0.link == URL(string: "https://muxy.app/docs") })
    }
}
