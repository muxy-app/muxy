import Carbon.HIToolbox
import Foundation
import SwiftUI
import Testing

@testable import Muxy

@Suite("Rich input transcript insertion")
@MainActor
struct RichInputTextEditorTests {
    @Test("insertion trims transcript and separates neighboring words")
    func insertionSeparatesNeighboringWords() {
        let result = RichInputTextEditor.preparedInsertion(
            "  spoken words\n",
            in: "beforeafter",
            replacing: NSRange(location: 6, length: 0)
        )

        #expect(result == " spoken words ")
    }

    @Test("insertion does not duplicate existing whitespace")
    func insertionUsesExistingWhitespace() {
        let result = RichInputTextEditor.preparedInsertion(
            "spoken words",
            in: "before  after",
            replacing: NSRange(location: 7, length: 0)
        )

        #expect(result == "spoken words")
    }

    @Test("insertion replaces the selected text")
    func insertionReplacesSelectedText() {
        let result = RichInputTextEditor.preparedInsertion(
            "replacement",
            in: "keep old value here",
            replacing: NSRange(location: 5, length: 9)
        )

        #expect(result == "replacement")
    }

    @Test("empty transcript inserts nothing")
    func emptyTranscriptInsertsNothing() {
        let result = RichInputTextEditor.preparedInsertion(
            " \n ",
            in: "existing",
            replacing: NSRange(location: 0, length: 0)
        )

        #expect(result.isEmpty)
    }

    @Test("Return finishes active dictation")
    func returnFinishesActiveDictation() {
        #expect(RichInputTextView.shouldFinishDictation(
            isDictating: true,
            keyCode: UInt16(kVK_Return),
            modifierFlags: []
        ))
        #expect(RichInputTextView.shouldFinishDictation(
            isDictating: true,
            keyCode: UInt16(kVK_ANSI_KeypadEnter),
            modifierFlags: [.numericPad]
        ))
    }

    @Test("Return keeps editing behavior outside active dictation")
    func returnKeepsEditingBehaviorOutsideActiveDictation() {
        #expect(!RichInputTextView.shouldFinishDictation(
            isDictating: false,
            keyCode: UInt16(kVK_Return),
            modifierFlags: []
        ))
        #expect(!RichInputTextView.shouldFinishDictation(
            isDictating: true,
            keyCode: UInt16(kVK_Return),
            modifierFlags: [.shift]
        ))
    }

    @Test("programmatic submission is delivered after representable update")
    func programmaticSubmissionIsDeferred() async {
        var text = ""
        var submitted = false
        let editor = RichInputTextEditor(
            text: Binding(
                get: { text },
                set: { text = $0 }
            ),
            callbacks: RichInputTextEditor.Callbacks(
                onSubmit: { _ in submitted = true }
            )
        )
        let coordinator = RichInputTextEditor.Coordinator(parent: editor)

        coordinator.applySubmissionIfNeeded(.init(id: UUID(), appendReturn: true))

        #expect(!submitted)
        await Task.yield()
        #expect(submitted)
    }
}
