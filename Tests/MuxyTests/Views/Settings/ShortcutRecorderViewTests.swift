import AppKit
import Testing

@testable import Muxy

@MainActor
@Suite("Shortcut recorder view")
struct ShortcutRecorderViewTests {
    @Test("resigning after a rejected shortcut cancels recording")
    func rejectedShortcutRemainsIncomplete() throws {
        let recorder = ShortcutRecorderNSView()
        var recordedCombo: KeyCombo?
        var cancellationCount = 0
        recorder.onRecord = { combo in
            recordedCombo = combo
            return false
        }
        recorder.onCancel = { cancellationCount += 1 }

        #expect(recorder.becomeFirstResponder())
        recorder.keyDown(with: try shortcutEvent())
        #expect(recorder.resignFirstResponder())

        #expect(recordedCombo == KeyCombo(key: "k", command: true))
        #expect(cancellationCount == 1)
    }

    @Test("resigning after an accepted shortcut does not cancel recording")
    func acceptedShortcutCompletesRecording() throws {
        let recorder = ShortcutRecorderNSView()
        var cancellationCount = 0
        recorder.onRecord = { _ in true }
        recorder.onCancel = { cancellationCount += 1 }

        #expect(recorder.becomeFirstResponder())
        recorder.keyDown(with: try shortcutEvent())
        #expect(recorder.resignFirstResponder())

        #expect(cancellationCount == 0)
    }

    private func shortcutEvent() throws -> NSEvent {
        try #require(NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: .command,
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: "k",
            charactersIgnoringModifiers: "k",
            isARepeat: false,
            keyCode: 40
        ))
    }
}
