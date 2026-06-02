import AppKit
import Testing

@testable import Muxy

@Suite("Extension dialog service")
@MainActor
struct ExtensionDialogServiceTests {
    @Test("dialog verbs are recognized and ungated")
    func dialogVerbsAreUngated() {
        let verbs = MuxyAPI.Permissions.verbNames
        #expect(verbs.contains("dialog.confirm"))
        #expect(verbs.contains("dialog.alert"))
        #expect(MuxyAPI.Permissions.required(for: "dialog.confirm") == nil)
        #expect(MuxyAPI.Permissions.required(for: "dialog.alert") == nil)
    }

    @Test("confirm parses fields and styles")
    func confirmParsesFields() throws {
        let request = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": "Delete branch?",
            "message": "Cannot be undone.",
            "buttons": ["Delete", "Cancel"],
            "cancel": "Cancel",
            "style": "warning",
        ])
        #expect(request.title == "Delete branch?")
        #expect(request.message == "Cannot be undone.")
        #expect(request.buttons == ["Delete", "Cancel"])
        #expect(request.cancelButton == "Cancel")
        #expect(request.style == .warning)
    }

    @Test("confirm moves the default button to the front")
    func confirmReordersDefault() throws {
        let request = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": "Proceed?",
            "buttons": ["Delete", "Cancel"],
            "default": "Cancel",
        ])
        #expect(request.buttons == ["Cancel", "Delete"])
        #expect(request.defaultButton == "Cancel")
    }

    @Test("confirm defaults buttons and drops empties")
    func confirmDefaultsButtons() throws {
        let fallback = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: ["title": "Hi"])
        #expect(fallback.buttons == ["OK", "Cancel"])

        let filtered = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": "Hi",
            "buttons": ["Yes", "", "No"],
        ])
        #expect(filtered.buttons == ["Yes", "No"])
    }

    @Test("confirm caps the button count")
    func confirmCapsButtonCount() throws {
        let request = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": "Pick",
            "buttons": ["A", "B", "C", "D", "E"],
        ])
        #expect(request.buttons.count == ExtensionDialogService.maxButtonCount)
        #expect(request.buttons == ["A", "B", "C"])
    }

    @Test("confirm clamps oversized text")
    func confirmClampsText() throws {
        let long = String(repeating: "x", count: ExtensionDialogService.maxTextLength + 500)
        let request = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": long,
            "message": long,
        ])
        #expect(request.title.count == ExtensionDialogService.maxTextLength)
        #expect(request.message.count == ExtensionDialogService.maxTextLength)
    }

    @Test("confirm requires title or message")
    func confirmRequiresContent() {
        #expect(throws: APIError.self) {
            try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [:])
        }
    }

    @Test("alert parses fields and defaults to informational")
    func alertParsesFields() throws {
        let request = try ExtensionDialogService.makeAlertRequest(extensionID: "ext", args: [
            "message": "Build finished",
        ])
        #expect(request.title.isEmpty)
        #expect(request.message == "Build finished")
        #expect(request.style == .informational)
    }

    @Test("alert maps critical style")
    func alertCriticalStyle() throws {
        let request = try ExtensionDialogService.makeAlertRequest(extensionID: "ext", args: [
            "title": "Failure",
            "style": "critical",
        ])
        #expect(request.style == .critical)
    }

    @Test("alert requires title or message")
    func alertRequiresContent() {
        #expect(throws: APIError.self) {
            try ExtensionDialogService.makeAlertRequest(extensionID: "ext", args: [:])
        }
    }

    @Test("Return maps to the first button and Esc to the cancel label")
    func keyEquivalentsMapReturnAndEscape() throws {
        let request = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": "Proceed?",
            "buttons": ["Discard", "Keep"],
            "default": "Keep",
            "cancel": "Discard",
        ])
        #expect(request.buttons == ["Keep", "Discard"])
        #expect(ExtensionDialogService.keyEquivalents(for: request) == ["\r", "\u{1B}"])
    }

    @Test("when default and cancel are the same label Return wins")
    func keyEquivalentsDefaultEqualsCancel() throws {
        let request = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": "Proceed?",
            "buttons": ["Delete", "Cancel"],
            "default": "Cancel",
            "cancel": "Cancel",
        ])
        #expect(request.buttons == ["Cancel", "Delete"])
        #expect(ExtensionDialogService.keyEquivalents(for: request) == ["\r", ""])
    }

    @Test("without a cancel label only the default button is bound")
    func keyEquivalentsDefaultOnly() throws {
        let request = try ExtensionDialogService.makeConfirmRequest(extensionID: "ext", args: [
            "title": "Proceed?",
            "buttons": ["Yes", "No"],
        ])
        #expect(ExtensionDialogService.keyEquivalents(for: request) == ["\r", ""])
    }
}
