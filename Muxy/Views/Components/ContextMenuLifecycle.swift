import AppKit
import ObjectiveC

final class ContextMenuLifecycle: NSObject, NSMenuDelegate {
    private weak var menu: NSMenu?

    func attach(to menu: NSMenu) {
        self.menu = menu
        menu.delegate = self
        objc_setAssociatedObject(menu, &Self.associationKey, self, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    func menuDidClose(_ menu: NSMenu) {
        releaseMenuItems(in: menu)
        menu.delegate = nil
        objc_setAssociatedObject(menu, &Self.associationKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }

    func releaseMenuItems(in menu: NSMenu) {
        for item in menu.items {
            item.target = nil
            item.representedObject = nil
            if let submenu = item.submenu {
                releaseMenuItems(in: submenu)
            }
        }
    }

    nonisolated(unsafe) private static var associationKey: UInt8 = 0
}
