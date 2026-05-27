import Foundation
import WebKit

final class ExtensionConsoleHandler: NSObject, WKScriptMessageHandler {
    static let messageHandlerName = "muxyConsole"

    private let extensionID: String

    init(extensionID: String) {
        self.extensionID = extensionID
    }

    func userContentController(
        _: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        guard let payload = message.body as? [String: Any] else { return }
        let level = (payload["level"] as? String) ?? "log"
        let text = (payload["message"] as? String) ?? ""
        guard !text.isEmpty else { return }
        ExtensionLogStore.shared.append(extensionID: extensionID, line: "[\(level)] \(text)")
    }
}
