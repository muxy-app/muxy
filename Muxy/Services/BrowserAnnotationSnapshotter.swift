import AppKit
import Foundation
import os
import WebKit

private let snapshotLogger = Logger(subsystem: "app.muxy", category: "BrowserAnnotationSnapshot")

@MainActor
enum BrowserAnnotationSnapshotter {
    private static let padding: CGFloat = 8

    static func capture(
        annotationID: UUID,
        rect: CGRect,
        pageZoom: CGFloat,
        webView: WKWebView,
        inspector: BrowserInspectorState,
        cacheStore: BrowserAnnotationCacheStore = .shared
    ) {
        guard let snapshotRect = clampedSnapshotRect(rect: rect, pageZoom: pageZoom, webView: webView) else { return }
        hideHighlight(in: webView) {
            let config = WKSnapshotConfiguration()
            config.rect = snapshotRect
            webView.takeSnapshot(with: config) { image, error in
                Task { @MainActor in
                    if let error {
                        snapshotLogger
                            .error("takeSnapshot failed: \(error.localizedDescription, privacy: .public)")
                        return
                    }
                    guard let image else { return }
                    guard let url = cacheStore.write(image: image, id: annotationID) else { return }
                    inspector.setScreenshotURL(url, for: annotationID)
                }
            }
        }
    }

    private static func clampedSnapshotRect(rect: CGRect, pageZoom: CGFloat, webView: WKWebView) -> CGRect? {
        guard rect.width > 0, rect.height > 0 else { return nil }
        let bounds = webView.bounds
        guard bounds.width > 0, bounds.height > 0 else { return nil }
        let scale = pageZoom > 0 ? pageZoom : 1
        let scaled = CGRect(
            x: rect.origin.x * scale,
            y: rect.origin.y * scale,
            width: rect.width * scale,
            height: rect.height * scale
        )
        let padded = scaled.insetBy(dx: -padding, dy: -padding)
        let clamped = padded.intersection(bounds)
        guard !clamped.isNull, clamped.width > 0, clamped.height > 0 else { return nil }
        return clamped
    }

    private static func hideHighlight(in webView: WKWebView, then completion: @escaping @MainActor () -> Void) {
        let script = "window.__muxyBrowserAPI && window.__muxyBrowserAPI.hideHighlight();"
        webView.evaluateJavaScript(script, in: nil, in: BrowserBridge.contentWorld) { _ in
            Task { @MainActor in
                completion()
            }
        }
    }
}
