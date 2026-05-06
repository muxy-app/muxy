import AppKit
import os
import SwiftUI
import UniformTypeIdentifiers
import WebKit

private let markdownWebLogger = Logger(subsystem: "app.muxy", category: "MarkdownWebView")

struct MarkdownPreviewScrollReport: Equatable {
    let scrollTop: CGFloat
    let scrollHeight: CGFloat
    let clientHeight: CGFloat

    var maxScrollTop: CGFloat { max(0, scrollHeight - clientHeight) }
}

struct MarkdownWebView: NSViewRepresentable {
    struct ContentUpdateRequest {
        let html: String
        let content: String
        let palette: MarkdownRenderer.Palette
        let syncScrollRequest: CGFloat?
        let syncScrollRequestVersion: Int
        let filePath: String?
        let projectPath: String?
        let fragmentTarget: String?
        let fragmentRequestVersion: Int
    }

    struct Configuration {
        let scrollSyncEnabled: Bool
        let syncScrollRequestVersion: Int
        let syncScrollRequest: CGFloat?
        let fragmentTarget: String?
        let fragmentRequestVersion: Int
        let onScrollReport: ((MarkdownPreviewScrollReport) -> Void)?
        let onLayoutChanged: (() -> Void)?
        let onAnchorGeometryChanged: (([MarkdownPreviewAnchorGeometry]) -> Void)?
        let onOpenInternalLink: ((String, String?) -> Void)?
    }

    let html: String
    let content: String
    let filePath: String?
    let projectPath: String?
    let palette: MarkdownRenderer.Palette
    @Binding var syncScrollRequest: CGFloat?
    let syncScrollRequestVersion: Int
    let fragmentTarget: String?
    let fragmentRequestVersion: Int
    var scrollSyncEnabled = true
    var onScrollReport: ((MarkdownPreviewScrollReport) -> Void)?
    var onLayoutChanged: (() -> Void)?
    var onAnchorGeometryChanged: (([MarkdownPreviewAnchorGeometry]) -> Void)?
    var onOpenInternalLink: ((String, String?) -> Void)?

    private var configuration: Configuration {
        Configuration(
            scrollSyncEnabled: scrollSyncEnabled,
            syncScrollRequestVersion: syncScrollRequestVersion,
            syncScrollRequest: syncScrollRequest,
            fragmentTarget: fragmentTarget,
            fragmentRequestVersion: fragmentRequestVersion,
            onScrollReport: onScrollReport,
            onLayoutChanged: onLayoutChanged,
            onAnchorGeometryChanged: onAnchorGeometryChanged,
            onOpenInternalLink: onOpenInternalLink
        )
    }

    private var contentUpdateRequest: ContentUpdateRequest {
        ContentUpdateRequest(
            html: html,
            content: content,
            palette: palette,
            syncScrollRequest: syncScrollRequest,
            syncScrollRequestVersion: syncScrollRequestVersion,
            filePath: filePath,
            projectPath: projectPath,
            fragmentTarget: fragmentTarget,
            fragmentRequestVersion: fragmentRequestVersion
        )
    }

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(
            MarkdownAssetSchemeHandler(),
            forURLScheme: MarkdownAssetSchemeHandler.scheme
        )
        config.setURLSchemeHandler(
            MarkdownLocalImageSchemeHandler(),
            forURLScheme: MarkdownLocalImageSchemeHandler.scheme
        )
        config.setURLSchemeHandler(
            MarkdownRemoteImageSchemeHandler(),
            forURLScheme: MarkdownRemoteImageSchemeHandler.scheme
        )
        context.coordinator.installBridge(into: config)

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator
        webView.wantsLayer = true
        webView.layer?.backgroundColor = palette.background.cgColor
        webView.setValue(false, forKey: "drawsBackground")
        webView.underPageBackgroundColor = palette.background
        context.coordinator.configure(with: configuration)
        if scrollSyncEnabled {
            context.coordinator.applyPreferredScroll(
                requestVersion: syncScrollRequestVersion,
                scrollTop: syncScrollRequest,
                to: webView
            )
        }

        context.coordinator.loadHTML(contentUpdateRequest, into: webView)
        return webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        context.coordinator.configure(with: configuration)
        context.coordinator.updateHTML(
            contentUpdateRequest,
            webView: webView
        )
    }

    static func dismantleNSView(_ webView: WKWebView, coordinator: Coordinator) {
        webView.navigationDelegate = nil
        webView.configuration.userContentController.removeScriptMessageHandler(forName: MarkdownWebBridge.scrollHandlerName)
        webView.configuration.userContentController.removeScriptMessageHandler(forName: MarkdownWebBridge.linkHandlerName)
        webView.configuration.userContentController
            .removeScriptMessageHandler(forName: MarkdownPreviewAnchorGeometryBridge.geometryHandlerName)
        coordinator.removeScrollObserver()
    }

    final class Coordinator: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
        private static let programmaticScrollSuppressionWindow: TimeInterval = 0.2

        private var lastHTML: String = ""
        private var lastAppliedPalette: MarkdownRenderer.Palette?
        private var pendingPalette: MarkdownRenderer.Palette?
        private var lastAppliedSyncRequestVersion: Int = -1
        private var lastReportedScrollTop: CGFloat = -1
        private var pendingSyncScrollTop: CGFloat?
        private var pendingSyncRequestVersion: Int = -1
        private var activeNavigation: WKNavigation?
        private var loadCount: Int = 0
        private var currentFilePath: String?
        private var currentProjectPath: String?
        private var lastRenderedContent: String = ""
        private var pendingContent: String?
        private var scrollSyncEnabled = true
        private var lastConfiguredScrollSyncEnabled = true
        private var pendingFragmentTarget: String?
        private var pendingFragmentRequestVersion: Int = -1
        private var lastAppliedFragmentRequestVersion: Int = -1
        private var onScrollReport: ((MarkdownPreviewScrollReport) -> Void)?
        private var onLayoutChanged: (() -> Void)?
        private var onAnchorGeometryChanged: (([MarkdownPreviewAnchorGeometry]) -> Void)?
        private var onOpenInternalLink: ((String, String?) -> Void)?
        private var isApplyingProgrammaticScroll = false
        private var isNavigationInFlight = false
        private var programmaticScrollSuppressionUntil: Date?
        private var lastAnchorGeometrySnapshot: [MarkdownPreviewAnchorGeometry] = []

        func configure(with configuration: Configuration) {
            scrollSyncEnabled = configuration.scrollSyncEnabled
            onScrollReport = configuration.onScrollReport
            onLayoutChanged = configuration.onLayoutChanged
            onAnchorGeometryChanged = configuration.onAnchorGeometryChanged
            onOpenInternalLink = configuration.onOpenInternalLink
        }

        func installBridge(into configuration: WKWebViewConfiguration) {
            configuration.userContentController.removeScriptMessageHandler(forName: MarkdownWebBridge.scrollHandlerName)
            configuration.userContentController.removeScriptMessageHandler(forName: MarkdownWebBridge.linkHandlerName)
            configuration.userContentController.removeScriptMessageHandler(forName: MarkdownPreviewAnchorGeometryBridge.geometryHandlerName)
            configuration.userContentController.removeAllUserScripts()
            configuration.userContentController.add(self, name: MarkdownWebBridge.scrollHandlerName)
            configuration.userContentController.add(self, name: MarkdownWebBridge.linkHandlerName)
            configuration.userContentController.add(self, name: MarkdownPreviewAnchorGeometryBridge.geometryHandlerName)
            configuration.userContentController.addUserScript(
                WKUserScript(
                    source: MarkdownWebBridge.scrollObserverScript,
                    injectionTime: .atDocumentEnd,
                    forMainFrameOnly: true
                )
            )
            configuration.userContentController.addUserScript(
                WKUserScript(
                    source: MarkdownWebBridge.linkObserverScript,
                    injectionTime: .atDocumentEnd,
                    forMainFrameOnly: true
                )
            )
            configuration.userContentController.addUserScript(
                WKUserScript(
                    source: MarkdownPreviewAnchorGeometryBridge.observerScript,
                    injectionTime: .atDocumentEnd,
                    forMainFrameOnly: true
                )
            )
        }

        func removeScrollObserver() {
            isApplyingProgrammaticScroll = false
            programmaticScrollSuppressionUntil = nil
        }

        func loadHTML(_ request: ContentUpdateRequest, into webView: WKWebView) {
            lastHTML = request.html
            currentFilePath = request.filePath
            currentProjectPath = request.projectPath
            pendingContent = request.content
            pendingPalette = request.palette
            pendingFragmentTarget = request.fragmentTarget
            pendingFragmentRequestVersion = request.fragmentTarget == nil ? -1 : request.fragmentRequestVersion
            lastAppliedPalette = nil
            lastRenderedContent = ""
            lastAppliedSyncRequestVersion = -1
            lastAppliedFragmentRequestVersion = -1
            lastReportedScrollTop = -1
            loadCount += 1
            isNavigationInFlight = true
            markdownWebLogger.debug(
                """
                Markdown web load seq=\(self.loadCount)
                path=\(request.filePath ?? "<nil>", privacy: .public)
                htmlLength=\(request.html.utf8.count)
                """
            )
            activeNavigation = webView.loadHTMLString(request.html, baseURL: nil)
        }

        func updateHTML(_ request: ContentUpdateRequest, webView: WKWebView) {
            let syncWasJustEnabled = scrollSyncEnabled && !lastConfiguredScrollSyncEnabled
            lastConfiguredScrollSyncEnabled = scrollSyncEnabled
            currentFilePath = request.filePath
            currentProjectPath = request.projectPath
            let fragmentRequestIsNew = request.fragmentTarget != nil
                && request.fragmentRequestVersion != lastAppliedFragmentRequestVersion
            if request.html != lastHTML {
                lastHTML = request.html
                pendingContent = request.content
                pendingPalette = request.palette
                pendingFragmentTarget = request.fragmentTarget
                pendingFragmentRequestVersion = request.fragmentTarget == nil ? -1 : request.fragmentRequestVersion
                lastAppliedPalette = nil
                lastRenderedContent = ""
                pendingSyncScrollTop = scrollSyncEnabled ? request.syncScrollRequest : nil
                pendingSyncRequestVersion = scrollSyncEnabled ? request.syncScrollRequestVersion : -1
                lastAppliedSyncRequestVersion = -1
                lastAppliedFragmentRequestVersion = -1
                loadCount += 1
                isNavigationInFlight = true
                markdownWebLogger.debug(
                    """
                    Markdown web update seq=\(self.loadCount)
                    path=\(request.filePath ?? "<nil>", privacy: .public)
                    htmlLength=\(request.html.utf8.count) pendingSyncRequestVersion=\(request.syncScrollRequestVersion)
                    """
                )
                activeNavigation = webView.loadHTMLString(request.html, baseURL: nil)
                return
            }
            if isNavigationInFlight {
                pendingContent = request.content
                pendingPalette = request.palette
                if fragmentRequestIsNew {
                    pendingFragmentTarget = request.fragmentTarget
                    pendingFragmentRequestVersion = request.fragmentRequestVersion
                }
                if scrollSyncEnabled {
                    pendingSyncScrollTop = request.syncScrollRequest
                    pendingSyncRequestVersion = request.syncScrollRequestVersion
                }
                return
            }

            applyPaletteIfNeeded(request.palette, to: webView)

            if request.content != lastRenderedContent {
                if fragmentRequestIsNew {
                    pendingFragmentTarget = request.fragmentTarget
                    pendingFragmentRequestVersion = request.fragmentRequestVersion
                }
                applyContentUpdate(
                    request.content,
                    to: webView,
                    reason: "swift-content-update"
                )
                if scrollSyncEnabled {
                    pendingSyncScrollTop = request.syncScrollRequest
                    pendingSyncRequestVersion = request.syncScrollRequestVersion
                }
                return
            } else if scrollSyncEnabled,
                      syncWasJustEnabled || request.syncScrollRequestVersion != lastAppliedSyncRequestVersion
            {
                applyPreferredScroll(
                    requestVersion: request.syncScrollRequestVersion,
                    scrollTop: request.syncScrollRequest,
                    to: webView
                )
            }
            if fragmentRequestIsNew {
                applyFragmentTarget(
                    request.fragmentTarget,
                    requestVersion: request.fragmentRequestVersion,
                    to: webView
                )
            }
        }

        private func applyPaletteIfNeeded(_ palette: MarkdownRenderer.Palette, to webView: WKWebView) {
            if let lastAppliedPalette, lastAppliedPalette == palette { return }
            lastAppliedPalette = palette
            webView.layer?.backgroundColor = palette.background.cgColor
            webView.underPageBackgroundColor = palette.background
            let script = MarkdownRenderer.themeApplyScript(palette: palette)
            webView.evaluateJavaScript(script) { _, error in
                if let error {
                    markdownWebLogger.error(
                        "Failed applying markdown theme: \(error.localizedDescription, privacy: .public)"
                    )
                }
            }
        }

        @MainActor
        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void
        ) {
            if navigationAction.navigationType == .linkActivated {
                if let url = navigationAction.request.url {
                    handleLinkActivation(url.absoluteString, webView: webView)
                }
                decisionHandler(.cancel)
                return
            }

            guard let url = navigationAction.request.url else {
                decisionHandler(.cancel)
                return
            }

            let scheme = url.scheme?.lowercased() ?? ""
            let isMainFrameInitialLoad = navigationAction.targetFrame?.isMainFrame == true
                && navigationAction.navigationType == .other
                && (scheme == "about" || url.absoluteString == "about:blank")

            if isMainFrameInitialLoad {
                decisionHandler(.allow)
                return
            }

            decisionHandler(.cancel)
        }

        private func handleLinkActivation(_ href: String, webView: WKWebView?) {
            switch MarkdownLinkResolver.resolve(
                href: href,
                currentFilePath: currentFilePath,
                projectPath: currentProjectPath
            ) {
            case let .external(url):
                NSWorkspace.shared.open(url)
            case let .internalFile(path, fragment):
                onOpenInternalLink?(path, fragment)
            case let .sameDocumentFragment(fragment):
                guard let webView else { return }
                applyFragmentTarget(
                    fragment,
                    requestVersion: lastAppliedFragmentRequestVersion + 1,
                    to: webView
                )
            case .unsupported:
                markdownWebLogger.debug("Ignored markdown link href=\(href, privacy: .private)")
            }
        }

        func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
            isNavigationInFlight = true
            markdownWebLogger.debug(
                "Markdown navigation didStart seq=\(self.loadCount) path=\(self.currentFilePath ?? "<nil>", privacy: .public)"
            )
        }

        private func applyContentUpdate(
            _ content: String,
            to webView: WKWebView,
            reason: String
        ) {
            let script = """
            \(MarkdownRenderer.updateScript(content: content))
            \(MarkdownPreviewAnchorGeometryBridge.requestMeasureScript(reason: reason))
            """
            webView.evaluateJavaScript(script) { [weak self] _, error in
                if let error {
                    markdownWebLogger.error(
                        "Failed updating markdown content in-place: \(error.localizedDescription, privacy: .public)"
                    )
                    return
                }

                guard let self else { return }
                self.lastRenderedContent = content
                self.collectJavaScriptErrors(from: webView)
                self.applyPendingFragmentTargetIfNeeded(to: webView)
                if self.scrollSyncEnabled,
                   let pendingSyncScrollTop = self.pendingSyncScrollTop,
                   self.pendingSyncRequestVersion >= 0
                {
                    let pendingRequestVersion = self.pendingSyncRequestVersion
                    self.pendingSyncScrollTop = nil
                    self.pendingSyncRequestVersion = -1
                    self.applyPreferredScroll(
                        requestVersion: pendingRequestVersion,
                        scrollTop: pendingSyncScrollTop,
                        to: webView
                    )
                }
            }
        }

        private func applyPendingFragmentTargetIfNeeded(to webView: WKWebView) {
            guard let pendingFragmentTarget, pendingFragmentRequestVersion >= 0 else { return }
            let pendingRequestVersion = pendingFragmentRequestVersion
            self.pendingFragmentTarget = nil
            pendingFragmentRequestVersion = -1
            applyFragmentTarget(
                pendingFragmentTarget,
                requestVersion: pendingRequestVersion,
                to: webView
            )
        }

        private func applyFragmentTarget(_ fragment: String?, requestVersion: Int, to webView: WKWebView) {
            guard let fragment, !fragment.isEmpty else { return }
            guard requestVersion != lastAppliedFragmentRequestVersion else { return }

            let script = MarkdownWebBridge.scrollToFragmentScript(fragment)
            webView.evaluateJavaScript(script) { [weak self] result, error in
                if let error {
                    markdownWebLogger.error(
                        "Failed scrolling markdown fragment: \(error.localizedDescription, privacy: .public)"
                    )
                    return
                }

                self?.lastAppliedFragmentRequestVersion = requestVersion
                if let didScroll = result as? Bool, !didScroll {
                    markdownWebLogger.debug("Markdown fragment target not found fragment=\(fragment, privacy: .private)")
                }
            }
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            if let navigation, let activeNavigation, navigation !== activeNavigation {
                markdownWebLogger.debug("Ignoring didFinish for stale markdown navigation")
                return
            }
            markdownWebLogger.debug(
                """
                Markdown navigation didFinish
                seq=\(self.loadCount)
                path=\(self.currentFilePath ?? "<nil>", privacy: .public)
                """
            )
            isNavigationInFlight = false
            lastAnchorGeometrySnapshot = []
            if let pendingPalette {
                self.pendingPalette = nil
                applyPaletteIfNeeded(pendingPalette, to: webView)
            }
            if let pendingContent {
                self.pendingContent = nil
                applyContentUpdate(
                    pendingContent,
                    to: webView,
                    reason: "swift-didFinish"
                )
            } else {
                applyPendingFragmentTargetIfNeeded(to: webView)
                if let pendingSyncScrollTop {
                    let pendingRequestVersion = pendingSyncRequestVersion
                    self.pendingSyncScrollTop = nil
                    pendingSyncRequestVersion = -1
                    applyPreferredScroll(
                        requestVersion: pendingRequestVersion,
                        scrollTop: pendingSyncScrollTop,
                        to: webView
                    )
                }
            }

            webView.evaluateJavaScript(
                MarkdownPreviewAnchorGeometryBridge.requestMeasureScript(reason: "swift-didFinish")
            ) { _, error in
                if let error {
                    markdownWebLogger.error(
                        "Failed requesting markdown anchor geometry: \(error.localizedDescription, privacy: .public)"
                    )
                }
            }
            collectJavaScriptErrors(from: webView)
        }

        func webView(
            _ webView: WKWebView,
            didFailProvisionalNavigation navigation: WKNavigation!,
            withError error: Error
        ) {
            isNavigationInFlight = false
            logNavigationFailure(kind: "provisional", navigation: navigation, error: error)
            collectJavaScriptErrors(from: webView)
        }

        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
            isNavigationInFlight = false
            logNavigationFailure(kind: "navigation", navigation: navigation, error: error)
            collectJavaScriptErrors(from: webView)
        }

        func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
            isNavigationInFlight = false
            markdownWebLogger.error(
                """
                Markdown web content process terminated
                path=\(self.currentFilePath ?? "<nil>", privacy: .public)
                reason=process-terminated
                """
            )
        }

        func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
            if message.name == MarkdownWebBridge.linkHandlerName {
                guard let payload = message.body as? [String: Any],
                      let href = payload["href"] as? String
                else { return }
                DispatchQueue.main.async {
                    self.handleLinkActivation(href, webView: nil)
                }
                return
            }

            if message.name == MarkdownPreviewAnchorGeometryBridge.geometryHandlerName {
                guard !isNavigationInFlight else { return }
                handleAnchorGeometryMessage(message.body)
                return
            }

            guard message.name == MarkdownWebBridge.scrollHandlerName,
                  scrollSyncEnabled,
                  !isNavigationInFlight,
                  let payload = message.body as? [String: Any],
                  let scrollTopNumber = payload["scrollTop"] as? NSNumber,
                  let scrollHeightNumber = payload["scrollHeight"] as? NSNumber,
                  let clientHeightNumber = payload["clientHeight"] as? NSNumber
            else { return }

            let scrollTop = CGFloat(truncating: scrollTopNumber)
            let scrollHeight = CGFloat(truncating: scrollHeightNumber)
            let clientHeight = CGFloat(truncating: clientHeightNumber)

            if let suppressionUntil = programmaticScrollSuppressionUntil, Date() < suppressionUntil {
                lastReportedScrollTop = scrollTop
                return
            }

            programmaticScrollSuppressionUntil = nil

            if isApplyingProgrammaticScroll, abs(scrollTop - lastReportedScrollTop) <= 0.5 {
                isApplyingProgrammaticScroll = false
                lastReportedScrollTop = scrollTop
                return
            }

            if isApplyingProgrammaticScroll {
                isApplyingProgrammaticScroll = false
                lastReportedScrollTop = scrollTop
                return
            }

            guard abs(lastReportedScrollTop - scrollTop) > 0.5 else { return }
            lastReportedScrollTop = scrollTop
            let report = MarkdownPreviewScrollReport(
                scrollTop: scrollTop,
                scrollHeight: scrollHeight,
                clientHeight: clientHeight
            )
            DispatchQueue.main.async {
                self.onScrollReport?(report)
            }
        }

        func applyPreferredScroll(
            requestVersion: Int,
            scrollTop: CGFloat?,
            to webView: WKWebView
        ) {
            guard let scrollTop else { return }
            guard requestVersion != lastAppliedSyncRequestVersion else { return }

            isApplyingProgrammaticScroll = true
            programmaticScrollSuppressionUntil = Date().addingTimeInterval(Self.programmaticScrollSuppressionWindow)
            let script = MarkdownWebBridge.scrollToTopScript(scrollTop)
            webView.evaluateJavaScript(script) { [weak self] _, error in
                if let error {
                    self?.isApplyingProgrammaticScroll = false
                    self?.programmaticScrollSuppressionUntil = nil
                    markdownWebLogger.error(
                        """
                        Failed applying markdown sync scroll
                        reason=\(error.localizedDescription, privacy: .public)
                        """
                    )
                    return
                }

                self?.lastAppliedSyncRequestVersion = requestVersion
            }
        }

        private func logNavigationFailure(kind: String, navigation: WKNavigation!, error: Error) {
            let nsError = error as NSError
            if let navigation, let activeNavigation, navigation !== activeNavigation {
                markdownWebLogger.debug(
                    """
                    Ignoring stale markdown \(kind, privacy: .public) failure
                    code=\(nsError.code) domain=\(nsError.domain, privacy: .public)
                    """
                )
                return
            }
            markdownWebLogger.error(
                """
                Markdown \(kind, privacy: .public) failure
                path=\(self.currentFilePath ?? "<nil>", privacy: .public)
                code=\(nsError.code) domain=\(nsError.domain, privacy: .public)
                reason=\(nsError.localizedDescription, privacy: .public)
                """
            )
        }

        private func collectJavaScriptErrors(from webView: WKWebView) {
            let script = """
            (() => {
                const entries = Array.isArray(window.__muxyErrors) ? window.__muxyErrors : [];
                window.__muxyErrors = [];
                return entries;
            })()
            """

            webView.evaluateJavaScript(script) { result, error in
                if let error {
                    markdownWebLogger.error(
                        """
                        Failed collecting markdown JavaScript errors
                        reason=\(error.localizedDescription, privacy: .public)
                        """
                    )
                    return
                }

                guard let entries = result as? [[String: Any]], !entries.isEmpty else {
                    return
                }

                for entry in entries {
                    let type = (entry["type"] as? String) ?? "unknown"
                    let message = (entry["message"] as? String) ?? ""
                    let source = (entry["source"] as? String) ?? ""
                    markdownWebLogger.error(
                        """
                        Markdown JavaScript \(type, privacy: .public)
                        message=\(message, privacy: .public)
                        source=\(source, privacy: .public)
                        """
                    )
                }
            }
        }

        private func handleAnchorGeometryMessage(_ body: Any) {
            guard let payload = body as? [String: Any],
                  let entries = payload["anchors"] as? [[String: Any]]
            else {
                return
            }

            let reason = (payload["reason"] as? String) ?? ""

            let geometries = entries.compactMap { entry -> MarkdownPreviewAnchorGeometry? in
                guard let anchorID = entry["anchorID"] as? String,
                      let topNumber = entry["top"] as? NSNumber,
                      let heightNumber = entry["height"] as? NSNumber
                else {
                    return nil
                }

                let startLine = (entry["startLine"] as? NSNumber)?.intValue
                let endLine = (entry["endLine"] as? NSNumber)?.intValue
                return MarkdownPreviewAnchorGeometry(
                    anchorID: anchorID,
                    startLine: startLine,
                    endLine: endLine,
                    top: CGFloat(truncating: topNumber),
                    height: CGFloat(truncating: heightNumber)
                )
            }.sorted(by: { lhs, rhs in
                if abs(lhs.top - rhs.top) > 0.5 {
                    return lhs.top < rhs.top
                }
                return lhs.anchorID < rhs.anchorID
            })

            guard geometrySnapshotIsMeaningfullyDifferent(from: lastAnchorGeometrySnapshot, to: geometries) else {
                return
            }

            lastAnchorGeometrySnapshot = geometries
            logAnchorGeometryIssuesIfNeeded(geometries)
            let shouldNotifyLayoutChange = shouldTriggerLayoutChange(forGeometryReason: reason)
            DispatchQueue.main.async {
                self.onAnchorGeometryChanged?(geometries)
                if shouldNotifyLayoutChange {
                    self.onLayoutChanged?()
                }
            }
        }

        private func shouldTriggerLayoutChange(forGeometryReason reason: String) -> Bool {
            let normalized = reason.lowercased()
            if normalized.isEmpty {
                return false
            }

            let noisyMarkers = ["img-load", "img-error", "resize-observer", "mutation", "connect"]
            if noisyMarkers.contains(where: { normalized.contains($0) }) {
                return false
            }

            let stableMarkers = ["swift-didfinish", "window-resize", "fonts-ready", "manual"]
            return stableMarkers.contains(where: { normalized.contains($0) })
        }

        private func geometrySnapshotIsMeaningfullyDifferent(
            from lhs: [MarkdownPreviewAnchorGeometry],
            to rhs: [MarkdownPreviewAnchorGeometry]
        ) -> Bool {
            if lhs.count != rhs.count {
                return true
            }

            for (left, right) in zip(lhs, rhs) {
                if left.anchorID != right.anchorID {
                    return true
                }
                if left.startLine != right.startLine || left.endLine != right.endLine {
                    return true
                }
                if abs(left.top - right.top) > 0.5 {
                    return true
                }
                if abs(left.height - right.height) > 0.5 {
                    return true
                }
            }

            return false
        }

        private func logAnchorGeometryIssuesIfNeeded(_ snapshot: [MarkdownPreviewAnchorGeometry]) {
            guard UserDefaults.standard.bool(forKey: "MuxyMarkdownAnchorGeometryDebug") else {
                return
            }

            if snapshot.isEmpty {
                markdownWebLogger.debug("Markdown anchor geometry snapshot empty")
                return
            }

            var previousTop: CGFloat?
            for geometry in snapshot {
                if let previousTop, geometry.top + 0.25 < previousTop {
                    markdownWebLogger.error(
                        "Markdown anchor geometry out of order anchorID=\(geometry.anchorID, privacy: .public) top=\(geometry.top)"
                    )
                    break
                }
                previousTop = geometry.top
            }

            let first = snapshot.first?.anchorID ?? "<nil>"
            let last = snapshot.last?.anchorID ?? "<nil>"
            markdownWebLogger.debug(
                "Markdown anchor geometry snapshot count=\(snapshot.count) first=\(first) last=\(last)"
            )
        }
    }
}
