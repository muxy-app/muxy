import AppKit
import Testing

@testable import Muxy

@Suite("ClosureMenuItem")
@MainActor
struct ClosureMenuItemTests {
    @Test("invokes its handler when activated")
    func invokesHandlerOnAction() {
        var invocations = 0
        let item = ClosureMenuItem(title: "Action") {
            invocations += 1
        }

        _ = item.target?.perform(item.action, with: item)

        #expect(invocations == 1)
    }

    @Test("captured target deallocates while menu retains the item")
    func captureDoesNotRetainTarget() {
        final class Owner {}

        weak var weakOwner: Owner?
        let menu = NSMenu(title: "Test")

        do {
            let owner = Owner()
            weakOwner = owner
            menu.addItem(ClosureMenuItem(title: "Action") { [weak owner] in
                _ = owner
            })
        }

        #expect(weakOwner == nil)
        #expect(menu.items.count == 1)
    }
}
