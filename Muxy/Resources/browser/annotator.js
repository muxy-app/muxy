(function () {
    if (window.top !== window) return;
    if (window.__muxyBrowserBridgeInstalled) return;
    window.__muxyBrowserBridgeInstalled = true;

    var STATE = {
        mode: 'off',
        hoveredEl: null,
        highlightEl: null,
        appliedTargets: [],
        suppressedSelectors: new Set([
            '#muxy-browser-highlight',
        ]),
    };

    function postMessage(name, payload) {
        try {
            window.webkit.messageHandlers.muxyBrowser.postMessage({
                name: name,
                payload: payload || {},
            });
        } catch (err) {
            return;
        }
    }

    function ensureHighlight() {
        if (STATE.highlightEl) return STATE.highlightEl;
        var el = document.createElement('div');
        el.id = 'muxy-browser-highlight';
        el.setAttribute('data-muxy-overlay', '');
        document.documentElement.appendChild(el);
        STATE.highlightEl = el;
        return el;
    }

    function hideHighlight() {
        if (!STATE.highlightEl) return;
        STATE.highlightEl.style.display = 'none';
    }

    function paintHighlight(rect) {
        var el = ensureHighlight();
        el.style.display = 'block';
        el.style.top = (rect.top + window.scrollY) + 'px';
        el.style.left = (rect.left + window.scrollX) + 'px';
        el.style.width = rect.width + 'px';
        el.style.height = rect.height + 'px';
    }

    function isOverlayElement(el) {
        if (!el) return true;
        var cursor = el;
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
        var parts = [];
        var cursor = el;
        while (cursor && cursor.nodeType === 1 && parts.length < 6) {
            var segment = cursor.nodeName.toLowerCase();
            if (cursor.id && /^[A-Za-z][A-Za-z0-9_-]*$/.test(cursor.id)) {
                segment = segment + '#' + cursor.id;
                parts.unshift(segment);
                break;
            }
            var classes = (cursor.getAttribute && cursor.getAttribute('class') || '')
                .trim()
                .split(/\s+/)
                .filter(function (c) { return c && /^[A-Za-z_-][A-Za-z0-9_-]*$/.test(c); })
                .slice(0, 2);
            if (classes.length) segment += '.' + classes.join('.');
            var parent = cursor.parentElement;
            if (parent) {
                var siblings = Array.from(parent.children).filter(function (s) { return s.nodeName === cursor.nodeName; });
                if (siblings.length > 1) {
                    var index = siblings.indexOf(cursor) + 1;
                    segment += ':nth-of-type(' + index + ')';
                }
            }
            parts.unshift(segment);
            cursor = cursor.parentElement;
        }
        return parts.join(' > ');
    }

    function isUniqueSelector(selector) {
        try {
            return document.querySelectorAll(selector).length === 1;
        } catch (err) {
            return false;
        }
    }

    function classSegment(el) {
        if (!el || !el.classList) return '';
        var classes = Array.from(el.classList)
            .filter(function (c) { return c && /^[A-Za-z_-][A-Za-z0-9_-]*$/.test(c); })
            .slice(0, 3);
        if (!classes.length) return '';
        return el.nodeName.toLowerCase() + '.' + classes.join('.');
    }

    function buildSelectorMinimal(el) {
        if (!(el instanceof Element)) return '';
        if (el.id && /^[A-Za-z][A-Za-z0-9_-]*$/.test(el.id)) {
            var idSelector = '#' + el.id;
            if (isUniqueSelector(idSelector)) return idSelector;
        }
        var leaf = classSegment(el) || el.nodeName.toLowerCase();
        if (isUniqueSelector(leaf)) return leaf;
        var cursor = el.parentElement;
        var chain = leaf;
        var depth = 0;
        while (cursor && cursor.nodeType === 1 && depth < 5) {
            var ancestor = '';
            if (cursor.id && /^[A-Za-z][A-Za-z0-9_-]*$/.test(cursor.id)) {
                ancestor = '#' + cursor.id;
            } else {
                ancestor = classSegment(cursor) || cursor.nodeName.toLowerCase();
            }
            chain = ancestor + ' ' + chain;
            if (isUniqueSelector(chain)) return chain;
            if (ancestor.indexOf('#') === 0) return chain;
            cursor = cursor.parentElement;
            depth += 1;
        }
        return '';
    }

    function outerHTMLSnippet(el) {
        if (!el || typeof el.outerHTML !== 'string') return '';
        return el.outerHTML;
    }

    function stylesheetHints(el) {
        if (!el || typeof el.matches !== 'function') return [];
        var sheets = document.styleSheets;
        if (!sheets || !sheets.length) return [];
        var hits = [];
        var seen = {};
        for (var i = 0; i < sheets.length && hits.length < 3; i++) {
            var sheet = sheets[i];
            var rules;
            try {
                rules = sheet.cssRules;
            } catch (err) {
                continue;
            }
            if (!rules) continue;
            var matched = false;
            for (var r = 0; r < rules.length; r++) {
                var rule = rules[r];
                if (!rule || typeof rule.selectorText !== 'string') continue;
                var selectors = rule.selectorText.split(',');
                for (var s = 0; s < selectors.length; s++) {
                    var candidate = selectors[s].trim();
                    if (!candidate) continue;
                    try {
                        if (el.matches(candidate)) { matched = true; break; }
                    } catch (err) {
                        continue;
                    }
                }
                if (matched) break;
            }
            if (!matched) continue;
            var href = sheet.href || '';
            if (!href || seen[href]) continue;
            seen[href] = true;
            hits.push(href);
        }
        return hits;
    }

    function documentDirection() {
        var dir = (document.documentElement && document.documentElement.dir) || '';
        if (dir) return dir;
        try {
            return window.getComputedStyle(document.documentElement).direction || '';
        } catch (err) {
            return '';
        }
    }

    function documentLanguage() {
        return (document.documentElement && document.documentElement.lang) || '';
    }

    function buildXPath(el) {
        if (!(el instanceof Element)) return '';
        var parts = [];
        var cursor = el;
        while (cursor && cursor.nodeType === 1 && cursor.nodeName.toLowerCase() !== 'html') {
            var name = cursor.nodeName.toLowerCase();
            var index = 1;
            var sibling = cursor.previousElementSibling;
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
        var trimmed = el.textContent.replace(/\s+/g, ' ').trim();
        if (trimmed.length <= 80) return trimmed;
        return trimmed.slice(0, 77) + '…';
    }

    function elementUnderPointer(event) {
        var el = event.target;
        if (isOverlayElement(el)) {
            el = document.elementFromPoint(event.clientX, event.clientY);
        }
        return el;
    }

    function handleMouseMove(event) {
        if (STATE.mode === 'off') return;
        var el = elementUnderPointer(event);
        if (!el || isOverlayElement(el)) {
            hideHighlight();
            return;
        }
        if (el === STATE.hoveredEl) return;
        STATE.hoveredEl = el;
        var rect = el.getBoundingClientRect();
        paintHighlight(rect);
    }

    function handleMouseLeave() {
        STATE.hoveredEl = null;
        hideHighlight();
    }

    function handleClick(event) {
        if (STATE.mode === 'off') return;
        var el = elementUnderPointer(event);
        if (!el || isOverlayElement(el)) return;
        event.preventDefault();
        event.stopPropagation();
        var rect = el.getBoundingClientRect();
        var computed = window.getComputedStyle(el);
        postMessage('picked', {
            selector: buildSelector(el),
            selectorMinimal: buildSelectorMinimal(el),
            xpath: buildXPath(el),
            textSnippet: textSnippet(el),
            outerHTML: outerHTMLSnippet(el),
            rect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height },
            viewport: { width: window.innerWidth, height: window.innerHeight },
            scroll: { x: window.scrollX, y: window.scrollY },
            url: window.location.href,
            title: document.title,
            dir: documentDirection(),
            lang: documentLanguage(),
            stylesheets: stylesheetHints(el),
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

    var ALLOWED_MODES = { off: true, pick: true };

    function setMode(mode) {
        var normalized = ALLOWED_MODES[mode] ? mode : 'off';
        STATE.mode = normalized;
        document.documentElement.setAttribute('data-muxy-mode', normalized);
        if (normalized === 'off') {
            STATE.hoveredEl = null;
            hideHighlight();
        }
    }

    var ALLOWED_PROPERTIES = new Set([
        'font-family',
        'font-size',
        'font-weight',
        'color',
        'background-color',
        'padding-top',
        'padding-right',
        'padding-bottom',
        'padding-left',
        'margin-top',
        'margin-right',
        'margin-bottom',
        'margin-left',
        'border-radius',
    ]);

    function clearAppliedOverrides() {
        for (var i = 0; i < STATE.appliedTargets.length; i++) {
            var entry = STATE.appliedTargets[i];
            try {
                for (var j = 0; j < entry.properties.length; j++) {
                    entry.element.style.removeProperty(entry.properties[j]);
                }
            } catch (err) {
                continue;
            }
        }
        STATE.appliedTargets = [];
    }

    function applyOverrides(rules) {
        clearAppliedOverrides();
        if (!Array.isArray(rules)) return;
        for (var i = 0; i < rules.length; i++) {
            var rule = rules[i];
            if (!rule || typeof rule.selector !== 'string') continue;
            var elements;
            try {
                elements = document.querySelectorAll(rule.selector);
            } catch (err) {
                continue;
            }
            var props = [];
            var declarations = Array.isArray(rule.declarations) ? rule.declarations : [];
            for (var d = 0; d < declarations.length; d++) {
                var decl = declarations[d];
                if (!decl || typeof decl.property !== 'string' || typeof decl.value !== 'string') continue;
                if (!ALLOWED_PROPERTIES.has(decl.property)) continue;
                props.push({ name: decl.property, value: decl.value });
            }
            if (props.length === 0) continue;
            for (var e = 0; e < elements.length; e++) {
                var element = elements[e];
                if (isOverlayElement(element)) continue;
                for (var p = 0; p < props.length; p++) {
                    try {
                        element.style.setProperty(props[p].name, props[p].value, 'important');
                    } catch (err) {
                        continue;
                    }
                }
                STATE.appliedTargets.push({
                    element: element,
                    properties: props.map(function (item) { return item.name; }),
                });
            }
        }
    }

    window.__muxyBrowserAPI = {
        setMode: setMode,
        applyOverrides: applyOverrides,
        clearOverrides: clearAppliedOverrides,
        hideHighlight: hideHighlight,
        scrollTo: function (y) {
            window.scrollTo(0, y);
        },
        getScroll: function () {
            return { x: window.scrollX, y: window.scrollY };
        },
    };

    function handleKeyDown(event) {
        if (STATE.mode === 'off') return;
        if (event.key !== 'Escape') return;
        event.preventDefault();
        event.stopPropagation();
        setMode('off');
        postMessage('inspectorDismissed', {});
    }

    document.addEventListener('mousemove', handleMouseMove, true);
    document.addEventListener('mouseleave', handleMouseLeave, true);
    document.addEventListener('click', handleClick, true);
    document.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('scroll', handleScroll, { passive: true });

    if (document.readyState === 'complete' || document.readyState === 'interactive') {
        postTitle();
    } else {
        document.addEventListener('DOMContentLoaded', postTitle);
    }

    var titleObserver = new MutationObserver(postTitle);
    var titleEl = document.querySelector('title');
    if (titleEl) titleObserver.observe(titleEl, { childList: true });
})();
