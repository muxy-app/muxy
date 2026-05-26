import AppKit
import Testing

@testable import Muxy

@Suite("ContextMenuLifecycle")
@MainActor
struct ContextMenuLifecycleTests {
    @Test("closing a menu releases item targets and represented objects")
    func closingMenuReleasesItemReferences() {
        let menu = NSMenu(title: "Test")
        let submenu = NSMenu(title: "Submenu")
        let target = NSObject()
        let representedObject = NSObject()
        let item = NSMenuItem(title: "Action", action: nil, keyEquivalent: "")
        let submenuItem = NSMenuItem(title: "Nested", action: nil, keyEquivalent: "")

        item.target = target
        item.representedObject = representedObject
        submenuItem.target = target
        submenuItem.representedObject = representedObject
        submenu.addItem(submenuItem)
        item.submenu = submenu
        menu.addItem(item)

        let lifecycle = ContextMenuLifecycle()
        lifecycle.attach(to: menu)
        lifecycle.menuDidClose(menu)

        #expect(item.target == nil)
        #expect(item.representedObject == nil)
        #expect(submenuItem.target == nil)
        #expect(submenuItem.representedObject == nil)
        #expect(menu.delegate == nil)
    }
}
