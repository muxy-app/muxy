import AppKit
import Foundation
import GhosttyKit
import Testing

@testable import Muxy

@MainActor
@Suite("SelectionClipboardWrite", .serialized)
struct SelectionClipboardWriteTests {
    @Test("shouldWriteSelectionClipboard is false when the setting is disabled")
    func helperFalseWhenDisabled() {
        #expect(!GhosttyRuntimeEventAdapter.shouldWriteSelectionClipboard(settingEnabled: false))
    }

    @Test("shouldWriteSelectionClipboard is true when the setting is enabled")
    func helperTrueWhenEnabled() {
        #expect(GhosttyRuntimeEventAdapter.shouldWriteSelectionClipboard(settingEnabled: true))
    }

    @Test("selection-clipboard write is a no-op when the setting is absent")
    func selectionWriteNoOpWhenAbsent() throws {
        try withControlledGlobals(autoCopy: nil) {
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString("muxy-before-absent", forType: .string)

            Self.writeToAdapter(location: GHOSTTY_CLIPBOARD_SELECTION, payload: "should-not-write")

            #expect(pasteboard.string(forType: .string) == "muxy-before-absent")
        }
    }

    @Test("selection-clipboard write is a no-op when the setting is disabled")
    func selectionWriteNoOpWhenDisabled() throws {
        try withControlledGlobals(autoCopy: false) {
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString("muxy-before-disabled", forType: .string)

            Self.writeToAdapter(location: GHOSTTY_CLIPBOARD_SELECTION, payload: "should-not-write")

            #expect(pasteboard.string(forType: .string) == "muxy-before-disabled")
        }
    }

    @Test("selection-clipboard write copies text when the setting is enabled")
    func selectionWriteCopiesWhenEnabled() throws {
        try withControlledGlobals(autoCopy: true) {
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()
            pasteboard.setString("muxy-before-enabled", forType: .string)

            Self.writeToAdapter(location: GHOSTTY_CLIPBOARD_SELECTION, payload: "copied-by-ghostty")

            #expect(pasteboard.string(forType: .string) == "copied-by-ghostty")
        }
    }

    @Test("standard-clipboard write ignores the auto-copy setting")
    func standardWriteIgnoresSetting() throws {
        try withControlledGlobals(autoCopy: false) {
            let pasteboard = NSPasteboard.general
            pasteboard.clearContents()

            Self.writeToAdapter(location: GHOSTTY_CLIPBOARD_STANDARD, payload: "standard-payload")

            #expect(pasteboard.string(forType: .string) == "standard-payload")
        }
    }

    @Test("selection-clipboard cleanup restores non-text pasteboard types")
    func cleanupPreservesNonTextPasteboardTypes() throws {
        let pasteboard = NSPasteboard.general
        let outerSnapshot = SystemPasteboardSnapshot.capture()
        defer { SystemPasteboardSnapshot.restore(items: outerSnapshot) }

        let pngData = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x6D, 0x75, 0x78, 0x79])
        pasteboard.clearContents()
        pasteboard.setData(pngData, forType: .png)
        pasteboard.setString("muxy-original-string", forType: .string)

        try withControlledGlobals(autoCopy: true) {
            Self.writeToAdapter(location: GHOSTTY_CLIPBOARD_SELECTION, payload: "copied-by-ghostty")

            #expect(pasteboard.string(forType: .string) == "copied-by-ghostty")
            #expect(pasteboard.data(forType: .png) == nil)
        }

        #expect(pasteboard.string(forType: .string) == "muxy-original-string")
        #expect(pasteboard.data(forType: .png) == pngData)
    }

    private static func writeToAdapter(location: ghostty_clipboard_e, payload: String) {
        let adapter = GhosttyRuntimeEventAdapter()
        payload.withCString { dataPtr in
            "text/plain".withCString { mimePtr in
                var item = ghostty_clipboard_content_s(mime: mimePtr, data: dataPtr)
                withUnsafePointer(to: &item) { itemPtr in
                    adapter.writeClipboard(location: location, content: itemPtr, len: 1)
                }
            }
        }
    }

    private func withControlledGlobals<T>(autoCopy: Bool?, _ body: () throws -> T) throws -> T {
        let key = GeneralSettingsKeys.autoCopyTerminalSelection
        let originalValue = UserDefaults.standard.object(forKey: key)
        let savedPasteboard = SystemPasteboardSnapshot.capture()

        if let autoCopy {
            UserDefaults.standard.set(autoCopy, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
        defer {
            if let originalValue {
                UserDefaults.standard.set(originalValue, forKey: key)
            } else {
                UserDefaults.standard.removeObject(forKey: key)
            }
            SystemPasteboardSnapshot.restore(items: savedPasteboard)
        }
        return try body()
    }
}
