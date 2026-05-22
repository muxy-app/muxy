import AppKit
import Foundation
import WebKit

@MainActor
final class BrowserBridge: NSObject, WKScriptMessageHandler {
    static let messageName = "muxyBrowser"

    weak var webView: BrowserWKWebView?
    private let state: BrowserTabState
    private let projectPath: String

    init(state: BrowserTabState, projectPath: String) {
        self.state = state
        self.projectPath = projectPath
    }

    nonisolated func userContentController(
        _: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        guard let payload = message.body as? [String: Any],
              let name = payload["name"] as? String
        else { return }
        let body = payload["payload"] as? [String: Any] ?? [:]
        Task { @MainActor in
            self.handleMessage(name: name, body: body)
        }
    }

    private func handleMessage(name: String, body: [String: Any]) {
        switch name {
        case "picked":
            handlePicked(body)
        case "hovered":
            state.hoveredSelector = body["selector"] as? String
        case "scrolled":
            if let y = body["y"] as? Double {
                state.scrollY = y
            }
        case "titleChanged":
            if let title = body["title"] as? String {
                state.pageTitle = title
            }
        default:
            break
        }
    }

    private func handlePicked(_ body: [String: Any]) {
        guard let selector = body["selector"] as? String,
              !selector.isEmpty
        else { return }
        let xpath = body["xpath"] as? String ?? ""
        let snippet = body["textSnippet"] as? String ?? ""
        let rectDict = body["rect"] as? [String: Double] ?? [:]
        let viewport = body["viewport"] as? [String: Double] ?? [:]
        let url = body["url"] as? String ?? state.currentURL
        let title = body["title"] as? String ?? state.pageTitle
        let computed = body["computedStyle"] as? [String: String] ?? [:]

        let rect = CGRect(
            x: rectDict["left"] ?? 0,
            y: rectDict["top"] ?? 0,
            width: rectDict["width"] ?? 0,
            height: rectDict["height"] ?? 0
        )

        let annotation = BrowserAnnotation(
            selector: selector,
            xpath: xpath,
            textSnippet: snippet,
            rect: rect,
            pageURL: url,
            pageTitle: title,
            viewportWidth: viewport["width"] ?? 0,
            viewportHeight: viewport["height"] ?? 0
        )

        state.addAnnotation(annotation)
        state.computedStyleSeeds[annotation.id] = computed

        if state.inspectorMode == .style {
            state.showsStyleInspector = true
        }
    }
}
