import Foundation

enum MarkdownPreviewAnchorGeometryBridge {
    static let geometryHandlerName = "muxyMarkdownAnchorGeometry"

    static let observerScript = #"""
    (() => {
        const handler = window.webkit?.messageHandlers?.muxyMarkdownAnchorGeometry;
        if (!handler) return;

        const scrollRoot = () => document.getElementById('content')
            || document.scrollingElement
            || document.documentElement
            || document.body;

        const readInt = value => {
            if (value === null || value === undefined) return null;
            const parsed = Number.parseInt(String(value), 10);
            return Number.isFinite(parsed) ? parsed : null;
        };

        const anchorSelector = '[data-muxy-anchor-id]';

        const measure = reason => {
            const root = scrollRoot();
            if (!root) return null;
            const rootRect = root.getBoundingClientRect();
            const nodes = Array.from(document.querySelectorAll(anchorSelector));
            const anchors = [];
            for (const node of nodes) {
                const anchorID = node.getAttribute('data-muxy-anchor-id');
                if (!anchorID) continue;
                const rect = node.getBoundingClientRect();
                const top = (rect.top - rootRect.top) + root.scrollTop;
                const height = rect.height;
                anchors.push({
                    anchorID,
                    startLine: readInt(node.getAttribute('data-muxy-line-start')),
                    endLine: readInt(node.getAttribute('data-muxy-line-end')),
                    top,
                    height,
                });
            }
            anchors.sort((a, b) => {
                const topDelta = a.top - b.top;
                if (Math.abs(topDelta) > 0.5) return topDelta;
                return String(a.anchorID).localeCompare(String(b.anchorID));
            });
            return {
                reason,
                timestamp: Date.now(),
                scrollTop: root.scrollTop,
                scrollHeight: root.scrollHeight,
                clientHeight: root.clientHeight,
                anchors,
            };
        };

        let lastAnchorPayload = null;
        let scheduled = false;
        let scheduleToken = 0;
        let pendingReason = null;

        const postIfChanged = reason => {
            const snapshot = measure(reason);
            if (!snapshot) return;
            const nextPayload = JSON.stringify(snapshot.anchors);
            if (nextPayload === lastAnchorPayload) return;
            lastAnchorPayload = nextPayload;
            handler.postMessage(snapshot);
        };

        const schedule = reason => {
            pendingReason = pendingReason ? `${pendingReason},${reason}` : reason;
            if (scheduled) return;
            scheduled = true;
            const token = ++scheduleToken;
            requestAnimationFrame(() => {
                if (token !== scheduleToken) return;
                scheduled = false;
                const reasonToSend = pendingReason || reason;
                pendingReason = null;
                postIfChanged(reasonToSend);
            });
        };

        const settle = reason => {
            schedule(reason);
            setTimeout(() => schedule(`${reason}:t50`), 50);
            setTimeout(() => schedule(`${reason}:t250`), 250);
        };

        const attachImageListeners = container => {
            const images = container.querySelectorAll('img');
            for (const image of images) {
                if (image.__muxyGeometryListenerAttached) continue;
                image.__muxyGeometryListenerAttached = true;
                image.addEventListener('load', () => settle('img-load'), { passive: true });
                image.addEventListener('error', () => settle('img-error'), { passive: true });
            }
        };

        let resizeObserver = null;
        let mutationObserver = null;

        const connect = () => {
            const root = scrollRoot();
            const markdownRoot = document.getElementById('markdown') || document.body;
            if (!root || !markdownRoot) return;

            attachImageListeners(markdownRoot);

            if (!resizeObserver && window.ResizeObserver) {
                resizeObserver = new ResizeObserver(() => schedule('resize-observer'));
                resizeObserver.observe(markdownRoot);
                resizeObserver.observe(root);
            }

            if (!mutationObserver && window.MutationObserver) {
                mutationObserver = new MutationObserver(mutations => {
                    if (!mutations || !mutations.length) return;
                    attachImageListeners(markdownRoot);
                    schedule('mutation');
                });
                mutationObserver.observe(markdownRoot, {
                    subtree: true,
                    childList: true,
                });
            }

            window.addEventListener('resize', () => settle('window-resize'), { passive: true });
            window.addEventListener('load', () => settle('window-load'), { passive: true });
            document.addEventListener('DOMContentLoaded', () => settle('dom-content-loaded'), { passive: true });

            if (document.fonts && document.fonts.ready) {
                document.fonts.ready.then(() => settle('fonts-ready')).catch(() => {});
            }

            settle('connect');
        };

        window.__muxyMeasureAnchors = reason => settle(reason || 'manual');
        connect();
    })();
    """#

    static func requestMeasureScript(reason: String) -> String {
        let escaped = reason
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
            .replacingOccurrences(of: "\n", with: "\\n")
        return """
        (() => {
            if (typeof window.__muxyMeasureAnchors === 'function') {
                window.__muxyMeasureAnchors("\(escaped)");
            }
        })();
        """
    }
}
