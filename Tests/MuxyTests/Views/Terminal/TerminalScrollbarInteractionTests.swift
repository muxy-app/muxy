import Foundation
import Testing

@testable import Muxy

@Suite("Terminal scrollbar interaction")
struct TerminalScrollbarInteractionTests {
    private let start = Date(timeIntervalSinceReferenceDate: 0)

    @Test("scroller is not interactive before any reveal")
    func idleByDefault() {
        let interaction = TerminalScrollbarInteraction()

        #expect(!interaction.allowsScrollerHit(now: start))
    }

    @Test("reveal opens an interaction window that expires")
    func revealWindowExpires() {
        var interaction = TerminalScrollbarInteraction()

        interaction.reveal(now: start)

        #expect(interaction.allowsScrollerHit(now: start))
        #expect(interaction.allowsScrollerHit(now: start.addingTimeInterval(1.0)))
        #expect(!interaction.allowsScrollerHit(now: start.addingTimeInterval(1.5)))
    }

    @Test("repeated reveals extend the interaction window")
    func revealExtendsWindow() {
        var interaction = TerminalScrollbarInteraction()

        interaction.reveal(now: start)
        interaction.reveal(now: start.addingTimeInterval(1.0))

        #expect(interaction.allowsScrollerHit(now: start.addingTimeInterval(2.0)))
        #expect(!interaction.allowsScrollerHit(now: start.addingTimeInterval(2.5)))
    }

    @Test("dragging keeps the scroller interactive past the reveal window")
    func draggingKeepsInteractive() {
        var interaction = TerminalScrollbarInteraction()

        interaction.reveal(now: start)
        interaction.beginDrag()

        #expect(interaction.isDragging)
        #expect(interaction.allowsScrollerHit(now: start.addingTimeInterval(60)))
    }

    @Test("ending a drag leaves a grace window before ownership returns")
    func endDragGraceWindow() {
        var interaction = TerminalScrollbarInteraction()

        interaction.beginDrag()
        interaction.endDrag(now: start.addingTimeInterval(60))

        #expect(!interaction.isDragging)
        #expect(interaction.allowsScrollerHit(now: start.addingTimeInterval(61)))
        #expect(!interaction.allowsScrollerHit(now: start.addingTimeInterval(62)))
    }
}
