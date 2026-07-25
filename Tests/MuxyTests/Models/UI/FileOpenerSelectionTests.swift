import Foundation
import Testing

@testable import Muxy

@Suite("FileOpenerSelection")
struct FileOpenerSelectionTests {
    @Test("options always include the independent top bar project target")
    func optionsAlwaysIncludeBuiltInTarget() {
        let options = FileOpenerSelection.options(from: [], selectedValue: FileOpenerSelection.builtinValue)

        #expect(options == [
            .init(
                id: FileOpenerSelection.builtinValue,
                title: "Built-in (Top Bar Project Target)",
                isAvailable: true
            ),
        ])
    }

    @Test("options include extension opener titles")
    func optionsIncludeExtensionOpeners() {
        let options = FileOpenerSelection.options(
            from: [
                binding(extensionName: "Editor", openerID: "code", openerTitle: "Source"),
                binding(extensionName: "Preview", openerID: "preview", openerTitle: nil),
            ],
            selectedValue: FileOpenerSelection.builtinValue
        )

        #expect(options.map(\.title) == [
            "Built-in (Top Bar Project Target)",
            "Editor (Source)",
            "Preview",
        ])
        #expect(options.allSatisfy { $0.isAvailable })
    }

    @Test("options preserve a temporarily unavailable extension selection")
    func optionsPreserveUnavailableSelection() {
        let selectedValue = FileOpenerSelection.value(extensionID: "editor", openerID: "code")
        let options = FileOpenerSelection.options(from: [], selectedValue: selectedValue)

        #expect(options.last == .init(
            id: selectedValue,
            title: "editor (code, unavailable)",
            isAvailable: false
        ))
    }

    @Test("options identify a malformed persisted selection as unavailable")
    func optionsIdentifyMalformedUnavailableSelection() {
        let options = FileOpenerSelection.options(from: [], selectedValue: "invalid")

        #expect(options.last == .init(
            id: "invalid",
            title: "Unavailable Extension Opener",
            isAvailable: false
        ))
    }

    private func binding(
        extensionName: String,
        openerID: String,
        openerTitle: String?
    ) -> ExtensionStore.FileOpenerBinding {
        let tabType = ExtensionTabType(
            id: "editor",
            title: "Editor",
            entry: "index.html",
            defaultData: nil
        )
        let opener = ExtensionFileOpener(
            id: openerID,
            title: openerTitle,
            tabType: tabType.id
        )
        let manifest = ExtensionManifest(
            name: extensionName,
            version: "1.0.0",
            tabTypes: [tabType],
            fileOpeners: [opener]
        )
        let muxyExtension = MuxyExtension(
            id: extensionName.lowercased(),
            directory: URL(fileURLWithPath: "/tmp/\(extensionName)"),
            manifest: manifest
        )
        return ExtensionStore.FileOpenerBinding(
            muxyExtension: muxyExtension,
            opener: opener,
            tabType: tabType
        )
    }
}
