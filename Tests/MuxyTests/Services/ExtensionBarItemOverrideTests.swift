import Foundation
import Testing

@testable import Muxy

@Suite("Extension bar item overrides")
@MainActor
struct ExtensionBarItemOverrideTests {
    @Test("ExtensionIcon.parse reads bare symbol strings")
    func parsesBareSymbol() {
        #expect(ExtensionIcon.parse("hammer.fill") == .symbol("hammer.fill"))
    }

    @Test("ExtensionIcon.parse reads symbol and svg objects")
    func parsesIconObjects() {
        #expect(ExtensionIcon.parse(["symbol": "bolt"]) == .symbol("bolt"))
        #expect(ExtensionIcon.parse(["svg": "badge.svg"]) == .svg("badge.svg"))
    }

    @Test("ExtensionIcon.parse rejects empty and malformed input")
    func parsesInvalidIcon() {
        #expect(ExtensionIcon.parse("") == nil)
        #expect(ExtensionIcon.parse(["symbol": ""]) == nil)
        #expect(ExtensionIcon.parse(["color": "red"]) == nil)
        #expect(ExtensionIcon.parse(nil) == nil)
    }

    @Test("topbar binding prefers the live icon over the manifest icon")
    func topbarDisplayIconFallback() {
        let item = ExtensionTopbarItem(id: "i", icon: .symbol("a"), tooltip: nil, command: "c")
        let base = ExtensionStore.TopbarItemBinding(muxyExtension: ext, item: item, liveIcon: nil)
        let overridden = ExtensionStore.TopbarItemBinding(muxyExtension: ext, item: item, liveIcon: .symbol("b"))
        #expect(base.displayIcon == .symbol("a"))
        #expect(overridden.displayIcon == .symbol("b"))
    }

    @Test("status bar binding prefers live icon and text over the manifest values")
    func statusBarDisplayFallback() {
        let item = ExtensionStatusBarItem(id: "i", icon: .symbol("a"), text: "1", tooltip: nil, side: .right, command: "c")
        let base = ExtensionStore.StatusBarItemBinding(muxyExtension: ext, item: item, liveIcon: nil, liveText: nil)
        let overridden = ExtensionStore.StatusBarItemBinding(
            muxyExtension: ext,
            item: item,
            liveIcon: .symbol("b"),
            liveText: "9"
        )
        #expect(base.displayIcon == .symbol("a"))
        #expect(base.displayText == "1")
        #expect(overridden.displayIcon == .symbol("b"))
        #expect(overridden.displayText == "9")
    }

    private var ext: MuxyExtension {
        MuxyExtension(
            id: "demo",
            directory: URL(fileURLWithPath: "/tmp/demo"),
            manifest: ExtensionManifest(name: "demo", version: "1.0.0", background: "background.js")
        )
    }
}
