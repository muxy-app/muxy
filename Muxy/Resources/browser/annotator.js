(function () {
    if (window.top !== window) return;
    if (window.__muxyBrowserBridgeInstalled) return;
    window.__muxyBrowserBridgeInstalled = true;

    const STATE = {
        mode: 'off',
        hoveredEl: null,
        highlightEl: null,
        overlayStyleEl: null,
        suppressedSelectors: new Set([
            '#muxy-browser-highlight',
            '#muxy-browser-style-overlay',
        ]),
    };

    function postMessage(name, payload) {
        try {
            window.webkit.messageHandlers.muxyBrowser.postMessage({
                name: name,
                payload: payload || {},
            });
        } catch (err) {
            // bridge not yet attached
        }
    }

    function ensureHighlight() {
        if (STATE.highlightEl) return STATE.highlightEl;
        const el = document.createElement('div');
        el.id = 'muxy-browser-highlight';
        el.setAttribute('data-muxy-overlay', '');
        document.documentElement.appendChild(el);
        STATE.highlightEl = el;
        return el;
    }

    function ensureStyleOverlay() {
        if (STATE.overlayStyleEl) return STATE.overlayStyleEl;
        const styleEl = document.createElement('style');
        styleEl.id = 'muxy-browser-style-overlay';
        styleEl.setAttribute('data-muxy-overlay', '');
        document.documentElement.appendChild(styleEl);
        STATE.overlayStyleEl = styleEl;
        return styleEl;
    }

    function hideHighlight() {
        if (!STATE.highlightEl) return;
        STATE.highlightEl.style.display = 'none';
    }

    function paintHighlight(rect) {
        const el = ensureHighlight();
        el.style.display = 'block';
        el.style.top = (rect.top + window.scrollY) + 'px';
        el.style.left = (rect.left + window.scrollX) + 'px';
        el.style.width = rect.width + 'px';
        el.style.height = rect.height + 'px';
    }

    function isOverlayElement(el) {
        if (!el) return true;
        let cursor = el;
        while (cursor) {
            if (cursor.hasAttribute && cursor.hasAttribute('data-muxy-overlay')) return true;
            cursor = cursor.parentElement;
        }
        return false;
    }

    function buildSelector(el) {
        if (!(el instanceof Element)) return '';
        if (el.id && /^[A-Za-z][A-Za-z0-9_-]*$/.test(el.id)) {
            return '#' + el.id;
        }
        const parts = [];
        let cursor = el;
        while (cursor && cursor.nodeType === 1 && parts.length < 6) {
            let segment = cursor.nodeName.toLowerCase();
            if (cursor.id && /^[A-Za-z][A-Za-z0-9_-]*$/.test(cursor.id)) {
                segment = segment + '#' + cursor.id;
                parts.unshift(segment);
                break;
            }
            const classes = (cursor.getAttribute && cursor.getAttribute('class') || '')
                .trim()
                .split(/\s+/)
                .filter(function (c) { return c && /^[A-Za-z_-][A-Za-z0-9_-]*$/.test(c); })
                .slice(0, 2);
            if (classes.length) segment += '.' + classes.join('.');
            const parent = cursor.parentElement;
            if (parent) {
                const siblings = Array.from(parent.children).filter(function (s) { return s.nodeName === cursor.nodeName; });
                if (siblings.length > 1) {
                    const index = siblings.indexOf(cursor) + 1;
                    segment += ':nth-of-type(' + index + ')';
                }
            }
            parts.unshift(segment);
            cursor = cursor.parentElement;
        }
        return parts.join(' > ');
    }

    function buildXPath(el) {
        if (!(el instanceof Element)) return '';
        const parts = [];
        let cursor = el;
        while (cursor && cursor.nodeType === 1 && cursor.nodeName.toLowerCase() !== 'html') {
            const name = cursor.nodeName.toLowerCase();
            let index = 1;
            let sibling = cursor.previousElementSibling;
            while (sibling) {
                if (sibling.nodeName === cursor.nodeName) index += 1;
                sibling = sibling.previousElementSibling;
            }
            parts.unshift(name + '[' + index + ']');
            cursor = cursor.parentElement;
        }
        return '/html/' + parts.join('/');
    }

    function textSnippet(el) {
        if (!el || !el.textContent) return '';
        const trimmed = el.textContent.replace(/\s+/g, ' ').trim();
        if (trimmed.length <= 80) return trimmed;
        return trimmed.slice(0, 77) + '…';
    }

    function elementUnderPointer(event) {
        let el = event.target;
        if (isOverlayElement(el)) {
            el = document.elementFromPoint(event.clientX, event.clientY);
        }
        return el;
    }

    function handleMouseMove(event) {
        if (STATE.mode === 'off') return;
        const el = elementUnderPointer(event);
        if (!el || isOverlayElement(el)) {
            hideHighlight();
            return;
        }
        if (el === STATE.hoveredEl) return;
        STATE.hoveredEl = el;
        const rect = el.getBoundingClientRect();
        paintHighlight(rect);
        postMessage('hovered', {
            selector: buildSelector(el),
        });
    }

    function handleMouseLeave() {
        STATE.hoveredEl = null;
        hideHighlight();
    }

    function handleClick(event) {
        if (STATE.mode === 'off') return;
        const el = elementUnderPointer(event);
        if (!el || isOverlayElement(el)) return;
        event.preventDefault();
        event.stopPropagation();
        const rect = el.getBoundingClientRect();
        const computed = window.getComputedStyle(el);
        postMessage('picked', {
            selector: buildSelector(el),
            xpath: buildXPath(el),
            textSnippet: textSnippet(el),
            rect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height },
            viewport: { width: window.innerWidth, height: window.innerHeight },
            scroll: { x: window.scrollX, y: window.scrollY },
            url: window.location.href,
            title: document.title,
            computedStyle: {
                fontFamily: computed.fontFamily,
                fontSize: computed.fontSize,
                fontWeight: computed.fontWeight,
                color: computed.color,
                backgroundColor: computed.backgroundColor,
                paddingTop: computed.paddingTop,
                paddingRight: computed.paddingRight,
                paddingBottom: computed.paddingBottom,
                paddingLeft: computed.paddingLeft,
                marginTop: computed.marginTop,
                marginRight: computed.marginRight,
                marginBottom: computed.marginBottom,
                marginLeft: computed.marginLeft,
                borderRadius: computed.borderRadius,
            },
        });
    }

    function handleScroll() {
        postMessage('scrolled', {
            x: window.scrollX,
            y: window.scrollY,
        });
        if (STATE.mode !== 'off') hideHighlight();
    }

    function postTitle() {
        postMessage('titleChanged', { title: document.title });
    }

    function setMode(mode) {
        STATE.mode = mode;
        document.documentElement.setAttribute('data-muxy-mode', mode);
        if (mode === 'off') {
            STATE.hoveredEl = null;
            hideHighlight();
        }
    }

    function applyOverrides(rules) {
        const styleEl = ensureStyleOverlay();
        styleEl.textContent = rules
            .map(function (rule) {
                return rule.selector + ' { ' + rule.declarations.map(function (decl) {
                    return decl.property + ': ' + decl.value + ' !important;';
                }).join(' ') + ' }';
            })
            .join('\n');
    }

    window.__muxyBrowserAPI = {
        setMode: setMode,
        applyOverrides: applyOverrides,
        clearOverrides: function () {
            if (STATE.overlayStyleEl) STATE.overlayStyleEl.textContent = '';
        },
        scrollTo: function (y) {
            window.scrollTo(0, y);
        },
        getScroll: function () {
            return { x: window.scrollX, y: window.scrollY };
        },
    };

    document.addEventListener('mousemove', handleMouseMove, true);
    document.addEventListener('mouseleave', handleMouseLeave, true);
    document.addEventListener('click', handleClick, true);
    window.addEventListener('scroll', handleScroll, { passive: true });

    if (document.readyState === 'complete' || document.readyState === 'interactive') {
        postTitle();
    } else {
        document.addEventListener('DOMContentLoaded', postTitle);
    }

    const titleObserver = new MutationObserver(postTitle);
    const titleEl = document.querySelector('title');
    if (titleEl) titleObserver.observe(titleEl, { childList: true });
})();
