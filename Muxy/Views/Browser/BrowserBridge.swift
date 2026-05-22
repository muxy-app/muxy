import AppKit
import Foundation
import os
import WebKit

private let bridgeLogger = Logger(subsystem: "app.muxy", category: "BrowserBridge")

@MainActor
final class BrowserBridge: NSObject, WKScriptMessageHandler {
    static let messageName = "muxyBrowser"
    static let contentWorld: WKContentWorld = .world(name: "muxyBrowserBridge")

    weak var webView: BrowserWKWebView?
    private let session: BrowserSession

    init(session: BrowserSession) {
        self.session = session
    }

    nonisolated func userContentController(
        _: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        Task { @MainActor in
            guard message.world == BrowserBridge.contentWorld else { return }
            guard let payload = message.body as? [String: Any],
                  let name = payload["name"] as? String
            else { return }
            let body = payload["payload"] as? [String: Any] ?? [:]
            self.handleMessage(name: name, body: body)
        }
    }

    private func handleMessage(name: String, body: [String: Any]) {
        switch name {
        case "picked":
            handlePicked(body)
        case "hovered":
            handleHovered(body)
        case "scrolled":
            handleScrolled(body)
        case "titleChanged":
            handleTitleChanged(body)
        default:
            bridgeLogger.debug("Ignoring unknown bridge message: \(name, privacy: .public)")
        }
    }

    private func handlePicked(_ body: [String: Any]) {
        guard session.inspector.inspectorMode != .off else { return }
        guard let selector = sanitizedString(
            body["selector"],
            maxLength: BrowserAnnotationSanitizer.maxSelectorLength
        ), !selector.isEmpty
        else { return }

        let xpath = sanitizedString(
            body["xpath"],
            maxLength: BrowserAnnotationSanitizer.maxXPathLength
        ) ?? ""
        let snippet = sanitizedString(
            body["textSnippet"],
            maxLength: BrowserAnnotationSanitizer.maxTextSnippetLength
        ) ?? ""
        let url = sanitizedString(
            body["url"],
            maxLength: BrowserAnnotationSanitizer.maxURLLength
        ) ?? session.nav.currentURL
        let title = sanitizedString(
            body["title"],
            maxLength: BrowserAnnotationSanitizer.maxTitleLength
        ) ?? session.nav.pageTitle

        let rect = parseRect(body["rect"])
        let viewport = parseViewport(body["viewport"])
        let computed = sanitizedComputedStyle(body["computedStyle"])

        let annotation = BrowserAnnotation(
            selector: selector,
            xpath: xpath,
            textSnippet: snippet,
            rect: rect,
            pageURL: url,
            pageTitle: title,
            viewportWidth: viewport.width,
            viewportHeight: viewport.height
        )

        session.inspector.addAnnotation(annotation)
        session.inspector.computedStyleSeeds[annotation.id] = computed

        if session.inspector.inspectorMode == .style {
            session.inspector.showsStyleInspector = true
        }
    }

    private func handleHovered(_ body: [String: Any]) {
        guard session.inspector.inspectorMode != .off else { return }
        session.inspector.hoveredSelector = sanitizedString(
            body["selector"],
            maxLength: BrowserAnnotationSanitizer.maxSelectorLength
        )
    }

    private func handleScrolled(_ body: [String: Any]) {
        guard let y = body["y"] as? Double else { return }
        session.nav.scrollY = y
    }

    private func handleTitleChanged(_ body: [String: Any]) {
        guard let title = sanitizedString(
            body["title"],
            maxLength: BrowserAnnotationSanitizer.maxTitleLength
        )
        else { return }
        session.nav.pageTitle = title
    }

    private func sanitizedString(_ value: Any?, maxLength: Int) -> String? {
        guard let raw = value as? String else { return nil }
        return BrowserAnnotationSanitizer.sanitizeSingleLine(raw, maxLength: maxLength)
    }

    private func parseRect(_ value: Any?) -> CGRect {
        guard let dict = value as? [String: Double] else { return .zero }
        return CGRect(
            x: clamp(dict["left"]),
            y: clamp(dict["top"]),
            width: clampNonNegative(dict["width"]),
            height: clampNonNegative(dict["height"])
        )
    }

    private func parseViewport(_ value: Any?) -> CGSize {
        guard let dict = value as? [String: Double] else { return .zero }
        return CGSize(
            width: clampNonNegative(dict["width"]),
            height: clampNonNegative(dict["height"])
        )
    }

    private func clamp(_ value: Double?) -> CGFloat {
        guard let value, value.isFinite else { return 0 }
        return CGFloat(max(-100_000, min(100_000, value)))
    }

    private func clampNonNegative(_ value: Double?) -> CGFloat {
        guard let value, value.isFinite, value > 0 else { return 0 }
        return CGFloat(min(100_000, value))
    }

    private func sanitizedComputedStyle(_ value: Any?) -> [String: String] {
        guard let dict = value as? [String: String] else { return [:] }
        var result: [String: String] = [:]
        result.reserveCapacity(dict.count)
        for (key, raw) in dict {
            let cleanedKey = BrowserAnnotationSanitizer.sanitizeSingleLine(
                key,
                maxLength: BrowserAnnotationSanitizer.maxStyleValueLength
            )
            let cleanedValue = BrowserAnnotationSanitizer.sanitizeSingleLine(
                raw,
                maxLength: BrowserAnnotationSanitizer.maxStyleValueLength
            )
            guard !cleanedKey.isEmpty else { continue }
            result[cleanedKey] = cleanedValue
        }
        return result
    }
}
