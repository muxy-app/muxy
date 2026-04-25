import CoreGraphics
import Foundation

enum MarkdownWebBridge {
    static let scrollHandlerName = "muxyMarkdownScroll"

    static let scrollObserverScript = #"""
    (() => {
        const handler = window.webkit?.messageHandlers?.muxyMarkdownScroll;
        if (!handler) return;

        let attachedRoot = null;
        let reportScheduled = false;

        const scrollRoot = () => document.getElementById('content')
            || document.scrollingElement
            || document.documentElement
            || document.body;

        const reportNow = () => {
            const root = scrollRoot();
            if (!root) return;
            handler.postMessage({
                scrollTop: root.scrollTop,
                scrollHeight: root.scrollHeight,
                clientHeight: root.clientHeight,
            });
        };

        const scheduleReport = () => {
            if (reportScheduled) return;
            reportScheduled = true;
            requestAnimationFrame(() => {
                reportScheduled = false;
                reportNow();
            });
        };

        const attach = () => {
            const root = scrollRoot();
            if (!root) return;
            if (attachedRoot === root) {
                scheduleReport();
                return;
            }
            if (attachedRoot) {
                attachedRoot.removeEventListener('scroll', scheduleReport);
            }
            attachedRoot = root;
            root.addEventListener('scroll', scheduleReport, { passive: true });
            scheduleReport();
        };

        window.addEventListener('resize', scheduleReport, { passive: true });
        window.addEventListener('load', () => setTimeout(attach, 0));
        document.addEventListener('DOMContentLoaded', () => setTimeout(attach, 0));
        setTimeout(attach, 0);
    })();
    """#

    static func scrollToTopScript(_ scrollTop: CGFloat) -> String {
        let target = max(0, scrollTop)
        return """
        (() => {
            const root = document.getElementById('content')
                || document.scrollingElement
                || document.documentElement
                || document.body;
            if (!root) return;
            const maxScrollTop = Math.max(0, root.scrollHeight - root.clientHeight);
            const target = Math.min(maxScrollTop, \(target));
            window.__muxyProgrammaticScroll = true;
            root.scrollTop = target;
            setTimeout(() => { window.__muxyProgrammaticScroll = false; }, 180);
        })();
        """
    }
}
