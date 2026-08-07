import AppKit

final class ClosureMenuItem: NSMenuItem {
    private let handler: () -> Void

    init(title: String, keyEquivalent: String = "", handler: @escaping () -> Void) {
        self.handler = handler
        super.init(title: title, action: #selector(invoke), keyEquivalent: keyEquivalent)
        target = self
    }

    convenience init(title: String, shortcut: KeyCombo, handler: @escaping () -> Void) {
        self.init(title: title, keyEquivalent: shortcut.nsKeyEquivalent, handler: handler)
        keyEquivalentModifierMask = shortcut.nsModifierFlags
    }

    @available(*, unavailable)
    required init(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    @objc
    private func invoke() {
        handler()
    }
}
