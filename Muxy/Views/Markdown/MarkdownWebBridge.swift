import CoreGraphics
import Foundation

enum MarkdownWebBridge {
    static let scrollHandlerName = "muxyMarkdownScroll"
    static let linkHandlerName = "muxyMarkdownLink"

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

    static let linkObserverScript = #"""
    (() => {
        const handler = window.webkit?.messageHandlers?.muxyMarkdownLink;

        const decodedFragment = (fragment) => {
            try {
                return decodeURIComponent(String(fragment || '').replace(/^#/, ''));
            } catch (_) {
                return String(fragment || '').replace(/^#/, '');
            }
        };

        const targetForFragment = (fragment) => {
            const id = decodedFragment(fragment);
            if (!id) return null;
            return document.getElementById(id) || document.getElementsByName(id)[0] || null;
        };

        window.__muxyScrollToMarkdownFragment = (fragment) => {
            const target = targetForFragment(fragment);
            if (!target) return false;
            target.scrollIntoView({ block: 'start' });
            return true;
        };

        document.addEventListener('click', (event) => {
            const target = event.target;
            const anchor = target && typeof target.closest === 'function' ? target.closest('a[href]') : null;
            if (!anchor) return;

            const href = String(anchor.getAttribute('href') || '').trim();
            if (!href) return;

            if (href.startsWith('#')) {
                event.preventDefault();
                window.__muxyScrollToMarkdownFragment(href);
                return;
            }

            if (!handler) return;
            event.preventDefault();
            handler.postMessage({ href });
        }, true);
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

    static func scrollToFragmentScript(_ fragment: String) -> String {
        let encodedFragment = javaScriptStringLiteral(fragment)
        return """
        (() => {
            if (typeof window.__muxyScrollToMarkdownFragment !== 'function') {
                return false;
            }
            return window.__muxyScrollToMarkdownFragment(\(encodedFragment));
        })();
        """
    }

    private static func javaScriptStringLiteral(_ value: String) -> String {
        guard let data = try? JSONEncoder().encode(value),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "\"\""
        }
        return literal
    }
}
