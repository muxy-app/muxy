import AppKit
import WebKit

@MainActor
final class BrowserInspectableWebView: WKWebView, BrowserElementInspecting {
    private static let inspectElementTitle = "Inspect Element"
    private static let closeInspectorTitle = "Close Inspector"
    private static let developerExtrasKey = "developerExtrasEnabled"
    private static let setDeveloperExtrasSelector = NSSelectorFromString("_setDeveloperExtrasEnabled:")
    private static let inspectorKey = "_inspector"
    private static let inspectorSelector = NSSelectorFromString("_inspector")
    private static let connectSelector = NSSelectorFromString("connect")
    private static let showInspectorSelector = NSSelectorFromString("show")
    private static let hideInspectorSelector = NSSelectorFromString("hide")
    private static let attachInspectorSelector = NSSelectorFromString("attach")
    private static let setInspectorDelegateSelector = NSSelectorFromString("setDelegate:")
    private static let isInspectorConnectedSelector = NSSelectorFromString("isConnected")
    private static let isInspectorVisibleSelector = NSSelectorFromString("isVisible")
    private static let inspectorConnectedKey = "connected"
    private static let inspectorVisibleKey = "visible"

    static func enableInspection(in configuration: WKWebViewConfiguration) {
        let preferences = configuration.preferences
        guard preferences.responds(to: Self.setDeveloperExtrasSelector) else { return }
        preferences.setValue(true, forKey: Self.developerExtrasKey)
    }

    static func inspectionEnabled(in configuration: WKWebViewConfiguration) -> Bool {
        configuration.preferences.value(forKey: Self.developerExtrasKey) as? Bool ?? false
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        let menu = super.menu(for: event) ?? NSMenu(title: "Browser")
        addInspectElementItem(to: menu)
        return menu
    }

    @discardableResult
    func inspectElement() -> Bool {
        guard isInspectable,
              let inspector,
              inspector.responds(to: Self.showInspectorSelector),
              inspector.responds(to: Self.attachInspectorSelector)
        else { return false }
        window?.makeFirstResponder(self)
        setInspectorDelegate(on: inspector)
        if isInspectorVisible(inspector) {
            hideInspector(inspector)
            return true
        }
        if isInspectorConnected(inspector) {
            openAttachedInspector(inspector)
        } else if inspector.responds(to: Self.connectSelector) {
            _ = inspector.perform(Self.connectSelector)
        } else {
            openAttachedInspector(inspector)
        }
        return true
    }

    @discardableResult
    func closeInspector() -> Bool {
        guard let inspector,
              inspector.responds(to: Self.hideInspectorSelector)
        else { return false }
        hideInspector(inspector)
        return true
    }

    func addInspectElementItem(to menu: NSMenu) {
        guard canOpenInspector else { return }
        guard Self.inspectorMenuItem(in: menu) == nil else { return }
        if !menu.items.isEmpty {
            menu.addItem(.separator())
        }
        let item = ClosureMenuItem(title: inspectElementItemTitle) { [weak self] in
            _ = self?.inspectElement()
        }
        menu.addItem(item)
    }

    private var canOpenInspector: Bool {
        guard isInspectable,
              let inspector
        else { return false }
        return inspector.responds(to: Self.showInspectorSelector)
            && inspector.responds(to: Self.attachInspectorSelector)
    }

    private var inspectElementItemTitle: String {
        guard let inspector, isInspectorVisible(inspector) else { return Self.inspectElementTitle }
        return Self.closeInspectorTitle
    }

    private static func inspectorMenuItem(in menu: NSMenu) -> NSMenuItem? {
        for item in menu.items {
            if item.title == inspectElementTitle || item.title == closeInspectorTitle {
                return item
            }
            if let submenu = item.submenu,
               let match = inspectorMenuItem(in: submenu)
            {
                return match
            }
        }
        return nil
    }

    private var inspector: AnyObject? {
        guard responds(to: Self.inspectorSelector) else { return nil }
        return value(forKey: Self.inspectorKey) as AnyObject?
    }

    private func setInspectorDelegate(on inspector: AnyObject) {
        guard inspector.responds(to: Self.setInspectorDelegateSelector) else { return }
        _ = inspector.perform(Self.setInspectorDelegateSelector, with: self)
    }

    private func isInspectorConnected(_ inspector: AnyObject) -> Bool {
        guard inspector.responds(to: Self.isInspectorConnectedSelector) else { return false }
        return inspector.value(forKey: Self.inspectorConnectedKey) as? Bool ?? false
    }

    private func isInspectorVisible(_ inspector: AnyObject) -> Bool {
        guard inspector.responds(to: Self.isInspectorVisibleSelector) else { return false }
        return inspector.value(forKey: Self.inspectorVisibleKey) as? Bool ?? false
    }

    private func openAttachedInspector(_ inspector: AnyObject) {
        attachInspector(inspector)
        showInspector(inspector)
        showInspector(inspector)
    }

    private func attachInspector(_ inspector: AnyObject) {
        guard inspector.responds(to: Self.attachInspectorSelector) else { return }
        _ = inspector.perform(Self.attachInspectorSelector)
    }

    private func showInspector(_ inspector: AnyObject) {
        guard inspector.responds(to: Self.showInspectorSelector) else { return }
        _ = inspector.perform(Self.showInspectorSelector)
    }

    private func hideInspector(_ inspector: AnyObject) {
        guard inspector.responds(to: Self.hideInspectorSelector) else { return }
        _ = inspector.perform(Self.hideInspectorSelector)
    }

    @objc(inspectorFrontendLoaded:)
    private func inspectorFrontendLoaded(_ inspector: AnyObject) {
        openAttachedInspector(inspector)
    }

    @objc(inspector:openURLExternally:)
    private func inspector(_: AnyObject, openURLExternally url: URL) {
        _ = NSWorkspace.shared.open(url)
    }
}
