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
        #expect(verbs.contains("dialog.prompt"))
        #expect(verbs.contains("dialog.pickFolder"))
        #expect(MuxyAPI.Permissions.required(for: "dialog.confirm") == nil)
        #expect(MuxyAPI.Permissions.required(for: "dialog.alert") == nil)
        #expect(MuxyAPI.Permissions.required(for: "dialog.prompt") == nil)
        #expect(MuxyAPI.Permissions.required(for: "dialog.pickFolder") == nil)
    }

    @Test("prompt parses fields and defaults the buttons")
    func promptParsesFields() throws {
        let request = try ExtensionDialogService.makePromptRequest(extensionID: "ext", args: [
            "title": "Rename worktree",
            "message": "New name",
            "default": "feature",
            "placeholder": "branch name",
        ])
        #expect(request.title == "Rename worktree")
        #expect(request.message == "New name")
        #expect(request.defaultValue == "feature")
        #expect(request.placeholder == "branch name")
        #expect(request.confirmButton == "OK")
        #expect(request.cancelButton == "Cancel")
    }

    @Test("prompt honors custom button labels and clamps text")
    func promptCustomButtonsAndClamp() throws {
        let long = String(repeating: "y", count: ExtensionDialogService.maxTextLength + 100)
        let request = try ExtensionDialogService.makePromptRequest(extensionID: "ext", args: [
            "title": "Q",
            "confirm": "Save",
            "cancel": "Discard",
            "default": long,
        ])
        #expect(request.confirmButton == "Save")
        #expect(request.cancelButton == "Discard")
        #expect(request.defaultValue.count == ExtensionDialogService.maxTextLength)
    }

    @Test("prompt requires title or message")
    func promptRequiresContent() {
        #expect(throws: APIError.self) {
            try ExtensionDialogService.makePromptRequest(extensionID: "ext", args: [:])
        }
    }

    @Test("pickFolder expands a tilde default path")
    func pickFolderExpandsTilde() throws {
        let request = try ExtensionDialogService.makePickFolderRequest(extensionID: "ext", args: [
            "title": "Choose project",
            "default": "~/Projects",
        ])
        #expect(request.title == "Choose project")
        #expect(request.defaultPath == ("~/Projects" as NSString).expandingTildeInPath)
    }

    @Test("pickFolder allows empty args")
    func pickFolderAllowsEmpty() throws {
        let request = try ExtensionDialogService.makePickFolderRequest(extensionID: "ext", args: [:])
        #expect(request.title.isEmpty)
        #expect(request.defaultPath == nil)
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
