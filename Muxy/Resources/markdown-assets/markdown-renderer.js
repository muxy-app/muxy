(function () {
    window.__muxyErrors = window.__muxyErrors || [];
    window.addEventListener('error', function (event) {
        try {
            var target = event && event.target;
            if (target && target.tagName === 'SCRIPT') {
                window.__muxyErrors.push({
                    type: 'script-load',
                    message: 'Failed to load script',
                    source: target.src || ''
                });
                return;
            }
            window.__muxyErrors.push({
                type: 'js-error',
                message: (event && event.message) ? String(event.message) : 'Unknown JavaScript error',
                source: (event && event.filename) ? String(event.filename) : ''
            });
        } catch (_) {}
    }, true);
    window.addEventListener('unhandledrejection', function (event) {
        try {
            var reason = event && event.reason;
            var message = (reason && reason.message) ? reason.message : String(reason || 'Unhandled rejection');
            window.__muxyErrors.push({
                type: 'unhandled-rejection',
                message: String(message),
                source: ''
            });
        } catch (_) {}
    });

    var _markedConfigured = false;

    function decodeBase64UTF8(base64) {
        try {
            var binary = atob(base64);
            var bytes = new Uint8Array(binary.length);
            for (var i = 0; i < binary.length; i++) {
                bytes[i] = binary.charCodeAt(i);
            }
            if (typeof TextDecoder !== 'undefined') {
                return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
            }
            var escaped = '';
            for (var j = 0; j < bytes.length; j++) {
                escaped += '%' + bytes[j].toString(16).padStart(2, '0');
            }
            return decodeURIComponent(escaped);
        } catch (_) {
            return '';
        }
    }

    function escapeHTML(value) {
        return String(value || '')
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    function sanitizeURL(rawValue, options) {
        var value = String(rawValue || '').trim();
        if (!value) {
            return null;
        }
        if (value.startsWith('#')) {
            return value;
        }
        var lower = value.toLowerCase();
        if (value.startsWith('//') || lower.startsWith('javascript:') || lower.startsWith('vbscript:')) {
            return null;
        }
        var allowData = Boolean(options && options.allowData);
        var allowBlob = Boolean(options && options.allowBlob);
        if (lower.startsWith('data:')) {
            return allowData ? value : null;
        }
        if (lower.startsWith('blob:')) {
            return allowBlob ? value : null;
        }
        var hasExplicitScheme = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(value);
        if (!hasExplicitScheme) {
            return value;
        }
        try {
            var resolved = new URL(value, document.baseURI);
            if (['http:', 'https:', 'mailto:', 'file:'].includes(resolved.protocol)) {
                return resolved.href;
            }
        } catch (_) {
            return null;
        }
        return null;
    }

    function sanitizeMarkdownDOM(markdownRoot) {
        if (!markdownRoot) {
            return;
        }
        var blockedTags = new Set([
            'script', 'iframe', 'object', 'embed', 'meta', 'link', 'style', 'base',
            'form', 'input', 'button', 'textarea', 'select', 'option', 'frame',
            'frameset', 'applet', 'svg', 'math'
        ]);
        var elements = Array.from(markdownRoot.querySelectorAll('*'));
        elements.forEach(function (element) {
            var tag = (element.tagName || '').toLowerCase();
            if (!tag) {
                return;
            }
            if (blockedTags.has(tag)) {
                element.remove();
                return;
            }
            Array.from(element.attributes).forEach(function (attribute) {
                var name = String(attribute.name || '').toLowerCase();
                if (!name) {
                    return;
                }
                if (name.startsWith('on') || name === 'srcdoc') {
                    element.removeAttribute(attribute.name);
                    return;
                }
                if (name === 'href') {
                    var safeHref = sanitizeURL(attribute.value, { allowData: false, allowBlob: false });
                    if (safeHref) {
                        element.setAttribute(attribute.name, safeHref);
                    } else {
                        element.removeAttribute(attribute.name);
                    }
                    return;
                }
                if (name === 'src') {
                    var isImageLike = ['img', 'source'].includes(tag);
                    var safeSrc = sanitizeURL(attribute.value, {
                        allowData: isImageLike,
                        allowBlob: isImageLike
                    });
                    if (safeSrc) {
                        element.setAttribute(attribute.name, safeSrc);
                    } else {
                        element.removeAttribute(attribute.name);
                    }
                    return;
                }
                if (name === 'xlink:href') {
                    element.removeAttribute(attribute.name);
                }
            });
        });
    }

    function loadScript(url) {
        return new Promise(function (resolve, reject) {
            var script = document.createElement('script');
            script.src = url;
            script.async = true;
            script.onload = function () { resolve(true); };
            script.onerror = function () { reject(new Error('Failed to load ' + url)); };
            document.head.appendChild(script);
        });
    }

    async function ensureMermaidLoaded() {
        if (typeof mermaid !== 'undefined') {
            return true;
        }
        var urls = ['muxy-asset://markdown/mermaid.min.js'];
        for (var i = 0; i < urls.length; i++) {
            try {
                await loadScript(urls[i]);
                if (typeof mermaid !== 'undefined') {
                    return true;
                }
            } catch (_) {}
        }
        return false;
    }

    function initializeMermaidControls() {
        var blocks = document.querySelectorAll('.mermaid');
        blocks.forEach(function (block) {
            var svg = block.querySelector('svg');
            if (!svg) {
                return;
            }
            var existingToolbars = block.querySelectorAll(':scope > .mermaid-toolbar');
            if (existingToolbars.length > 1) {
                for (var t = 1; t < existingToolbars.length; t++) {
                    existingToolbars[t].remove();
                }
            }
            var existingCanvases = block.querySelectorAll(':scope > .mermaid-canvas');
            if (existingCanvases.length > 1) {
                for (var c = 1; c < existingCanvases.length; c++) {
                    existingCanvases[c].remove();
                }
            }
            var toolbar = existingToolbars[0] || null;
            var canvas = existingCanvases[0] || null;
            if (block.dataset.controlsReady === 'true' && toolbar && canvas && canvas.contains(svg)) {
                applyMermaidState(block, svg, toolbar);
                return;
            }
            if (toolbar) {
                toolbar.remove();
            }
            if (!(svg.parentElement && svg.parentElement.classList.contains('mermaid-canvas'))) {
                if (!canvas) {
                    canvas = document.createElement('div');
                    canvas.className = 'mermaid-canvas';
                }
                if (svg.parentNode) {
                    svg.parentNode.insertBefore(canvas, svg);
                } else {
                    block.appendChild(canvas);
                }
                canvas.appendChild(svg);
            } else {
                canvas = svg.parentElement;
            }
            toolbar = document.createElement('div');
            toolbar.className = 'mermaid-toolbar';
            toolbar.innerHTML = ''
                + '<button class="mermaid-btn" data-action="toggle-size">Natural</button>'
                + '<button class="mermaid-btn" data-action="zoom-out">-</button>'
                + '<span class="mermaid-zoom-label">100%</span>'
                + '<button class="mermaid-btn" data-action="zoom-in">+</button>'
                + '<button class="mermaid-btn" data-action="zoom-reset">Reset</button>';
            block.insertBefore(toolbar, canvas);
            var viewBox = (svg.getAttribute('viewBox') || '').trim().split(/\s+/);
            var naturalWidth = parseFloat(svg.getAttribute('width'));
            if (!isFinite(naturalWidth) || naturalWidth <= 0) {
                naturalWidth = viewBox.length === 4 ? parseFloat(viewBox[2]) : NaN;
            }
            if (!isFinite(naturalWidth) || naturalWidth <= 0) {
                naturalWidth = Math.max(400, svg.getBoundingClientRect().width || 800);
            }
            block.dataset.controlsReady = 'true';
            block.dataset.sizeMode = 'fit';
            block.dataset.zoom = '1';
            block.dataset.naturalWidth = String(naturalWidth);
            toolbar.addEventListener('click', function (event) {
                var target = event.target;
                var actionEl = null;
                if (target && typeof target.closest === 'function') {
                    actionEl = target.closest('[data-action]');
                }
                if (!actionEl) {
                    return;
                }
                var action = actionEl.dataset.action;
                if (!action) {
                    return;
                }
                event.preventDefault();
                var zoom = parseFloat(block.dataset.zoom || '1');
                var mode = block.dataset.sizeMode || 'fit';
                if (action === 'toggle-size') {
                    mode = mode === 'fit' ? 'natural' : 'fit';
                    block.dataset.sizeMode = mode;
                } else if (action === 'zoom-in') {
                    zoom = Math.min(4, zoom + 0.1);
                } else if (action === 'zoom-out') {
                    zoom = Math.max(0.3, zoom - 0.1);
                } else if (action === 'zoom-reset') {
                    zoom = 1;
                }
                block.dataset.zoom = String(zoom);
                applyMermaidState(block, svg, toolbar);
            });
            applyMermaidState(block, svg, toolbar);
        });
    }

    function applyMermaidState(block, svg, toolbar) {
        var mode = block.dataset.sizeMode || 'fit';
        var zoom = parseFloat(block.dataset.zoom || '1');
        var naturalWidth = parseFloat(block.dataset.naturalWidth || '800');
        var toggleBtn = toolbar.querySelector('[data-action="toggle-size"]');
        var zoomLabel = toolbar.querySelector('.mermaid-zoom-label');
        if (mode === 'fit') {
            svg.style.width = (zoom * 100).toFixed(1).replace('.0', '') + '%';
            svg.style.maxWidth = 'none';
        } else {
            svg.style.width = Math.max(120, naturalWidth * zoom) + 'px';
            svg.style.maxWidth = 'none';
        }
        svg.style.height = 'auto';
        if (toggleBtn) {
            toggleBtn.textContent = mode === 'fit' ? 'Natural' : 'Fit';
        }
        if (zoomLabel) {
            zoomLabel.textContent = Math.round(zoom * 100) + '%';
        }
    }

    function detectAnchorKind(lines, index) {
        var line = lines[index] || '';
        var trimmed = line.trim();
        if (!trimmed) {
            return null;
        }
        if (/^ {0,3}(?:```+|~~~+)/.test(line)) {
            var info = line.replace(/^ {0,3}(?:```+|~~~+)\s*/, '').trim().toLowerCase();
            return info === 'mermaid' ? 'mermaid' : 'fencedCode';
        }
        if (/^ {0,3}#{1,6}(?:\s+|$)/.test(line)) {
            return 'heading';
        }
        if (/^ {0,3}(?:[-*_])(?:\s*[-*_]){2,}\s*$/.test(line)) {
            return 'thematicBreak';
        }
        if (/^ {0,3}>\s?/.test(line)) {
            return 'blockquote';
        }
        if (/^ {0,3}(?:[*+-]|\d+[.)])\s+/.test(line)) {
            return 'list';
        }
        if (/^\s*!\[[^\]]*\]\([^\)]+\)\s*$/.test(line)) {
            return 'image';
        }
        if (/\|/.test(line)) {
            var next = lines[index + 1] || '';
            if (/^\s*\|?(?:\s*:?-{3,}:?\s*\|)+\s*:?-{3,}:?\s*\|?\s*$/.test(next)) {
                return 'table';
            }
        }
        if (/^ {0,3}<(?!!--)([A-Za-z][\w-]*)(\s|>|$)/.test(line)) {
            return 'htmlBlock';
        }
        return 'paragraph';
    }

    function consumeAnchor(lines, index, kind) {
        var i = index;
        if (kind === 'heading' || kind === 'thematicBreak' || kind === 'image' || kind === 'htmlBlock') {
            return index;
        }
        if (kind === 'fencedCode' || kind === 'mermaid') {
            var opener = lines[index] || '';
            var openerMatch = opener.match(/^ {0,3}(```+|~~~+)/);
            var fence = openerMatch ? openerMatch[1][0] : '`';
            var minCount = openerMatch ? openerMatch[1].length : 3;
            i = index + 1;
            while (i < lines.length) {
                var candidate = lines[i] || '';
                var closeMatch = candidate.match(/^ {0,3}(```+|~~~+)\s*$/);
                if (closeMatch && closeMatch[1][0] === fence && closeMatch[1].length >= minCount) {
                    return i;
                }
                i += 1;
            }
            return lines.length - 1;
        }
        if (kind === 'blockquote') {
            while (i + 1 < lines.length) {
                var nextLine = lines[i + 1] || '';
                if (!nextLine.trim()) {
                    i += 1;
                    continue;
                }
                if (!/^ {0,3}>\s?/.test(nextLine)) {
                    break;
                }
                i += 1;
            }
            return i;
        }
        if (kind === 'list') {
            while (i + 1 < lines.length) {
                var listNext = lines[i + 1] || '';
                if (!listNext.trim()) {
                    i += 1;
                    continue;
                }
                if (/^ {0,3}(?:[*+-]|\d+[.)])\s+/.test(listNext) || /^\s{2,}\S/.test(listNext)) {
                    i += 1;
                    continue;
                }
                break;
            }
            return i;
        }
        if (kind === 'table') {
            i = index + 1;
            while (i + 1 < lines.length) {
                var tableNext = lines[i + 1] || '';
                if (!tableNext.trim() || !/\|/.test(tableNext)) {
                    break;
                }
                i += 1;
            }
            return i;
        }
        while (i + 1 < lines.length) {
            var paragraphNext = lines[i + 1] || '';
            if (!paragraphNext.trim()) {
                break;
            }
            if (detectAnchorKind(lines, i + 1) !== 'paragraph') {
                break;
            }
            i += 1;
        }
        return i;
    }

    function parseSyncAnchors(content) {
        var lines = content.split(/\r?\n/);
        var anchors = [];
        var i = 0;
        var sequence = 0;
        while (i < lines.length) {
            if (!(lines[i] || '').trim()) {
                i += 1;
                continue;
            }
            var kind = detectAnchorKind(lines, i) || 'other';
            var end = consumeAnchor(lines, i, kind);
            var startLine = i + 1;
            var endLine = end + 1;
            anchors.push({
                id: 'muxy-anchor-' + String(sequence),
                kind: kind,
                startLine: startLine,
                endLine: Math.max(startLine, endLine)
            });
            sequence += 1;
            i = end + 1;
        }
        return anchors;
    }

    function inferElementKind(element) {
        if (!element) {
            return 'other';
        }
        var tag = (element.tagName || '').toLowerCase();
        if (/^h[1-6]$/.test(tag)) {
            return 'heading';
        }
        if (tag === 'ul' || tag === 'ol') {
            return 'list';
        }
        if (tag === 'blockquote') {
            return 'blockquote';
        }
        if (tag === 'pre') {
            return 'fencedCode';
        }
        if (tag === 'table') {
            return 'table';
        }
        if (tag === 'hr') {
            return 'thematicBreak';
        }
        if (tag === 'img') {
            return 'image';
        }
        if (tag === 'div' && element.classList.contains('mermaid')) {
            return 'mermaid';
        }
        if (tag === 'p') {
            var meaningfulNodes = Array.prototype.slice.call(element.childNodes).filter(function (node) {
                if (node.nodeType === Node.TEXT_NODE) {
                    return Boolean((node.textContent || '').trim());
                }
                return true;
            });
            if (meaningfulNodes.length === 1
                && meaningfulNodes[0].nodeType === Node.ELEMENT_NODE
                && meaningfulNodes[0].tagName
                && meaningfulNodes[0].tagName.toLowerCase() === 'img') {
                return 'image';
            }
            return 'paragraph';
        }
        return 'other';
    }

    function collectAnchorElements(root) {
        return Array.prototype.slice.call(root.children).filter(function (element) {
            if (!element || !element.tagName) {
                return false;
            }
            var tag = element.tagName.toLowerCase();
            if (/^h[1-6]$/.test(tag)) {
                return true;
            }
            return ['p', 'ul', 'ol', 'blockquote', 'pre', 'table', 'hr', 'img', 'div'].includes(tag);
        });
    }

    function slugifyHeading(value) {
        return String(value || '')
            .trim()
            .toLowerCase()
            .replace(/[^\w\s-]/g, '')
            .replace(/\s+/g, '-')
            .replace(/-+/g, '-')
            .replace(/^-|-$/g, '');
    }

    function assignHeadingIDs(markdownRoot) {
        if (!markdownRoot) {
            return;
        }
        var counts = Object.create(null);
        markdownRoot.querySelectorAll('h1, h2, h3, h4, h5, h6').forEach(function (heading) {
            if (heading.id) {
                return;
            }
            var base = slugifyHeading(heading.textContent) || 'heading';
            var count = counts[base] || 0;
            counts[base] = count + 1;
            heading.id = count === 0 ? base : base + '-' + String(count);
        });
    }

    function encodeRemoteURL(rawUrl) {
        try {
            var binary = '';
            var bytes = new TextEncoder().encode(rawUrl);
            for (var i = 0; i < bytes.length; i++) {
                binary += String.fromCharCode(bytes[i]);
            }
            return btoa(binary)
                .replace(/\+/g, '-')
                .replace(/\//g, '_')
                .replace(/=+$/, '');
        } catch (_) {
            return null;
        }
    }

    function rewriteImageSource(image) {
        var rawSrc = image.getAttribute('src');
        if (!rawSrc) {
            return;
        }
        var trimmed = rawSrc.trim();
        if (!trimmed) {
            return;
        }
        var lower = trimmed.toLowerCase();
        if (lower.startsWith('data:') || lower.startsWith('blob:')) {
            return;
        }
        if (lower.startsWith('https://')) {
            var encoded = encodeRemoteURL(trimmed);
            if (encoded) {
                image.setAttribute('src', 'muxy-md-remote://image/' + encoded);
            }
            return;
        }
        if (lower.startsWith('http://')) {
            image.removeAttribute('src');
            return;
        }
        var hasScheme = /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(trimmed);
        if (hasScheme || trimmed.startsWith('//')) {
            return;
        }
        var baseHost = window.__muxyImageBaseHost || '';
        if (!baseHost) {
            return;
        }
        var relative = trimmed.replace(/^\/+/, '');
        var encodedRelative = relative.split('/').map(encodeURIComponent).join('/');
        image.setAttribute('src', 'muxy-md-image://' + baseHost + '/' + encodedRelative);
    }

    function normalizeLocalImageSources(markdownRoot) {
        if (!markdownRoot) {
            return;
        }
        markdownRoot.querySelectorAll('img[src]').forEach(rewriteImageSource);
    }

    function assignAnchorMetadata(markdownRoot, anchors) {
        if (!markdownRoot || !anchors || !anchors.length) {
            return;
        }
        var elements = collectAnchorElements(markdownRoot);
        var anchorIndex = 0;
        for (var i = 0; i < elements.length && anchorIndex < anchors.length; i++) {
            var element = elements[i];
            var elementKind = inferElementKind(element);
            var selectedIndex = anchorIndex;
            for (var lookahead = anchorIndex; lookahead < anchors.length; lookahead++) {
                if (anchors[lookahead].kind === elementKind) {
                    selectedIndex = lookahead;
                    break;
                }
            }
            var anchor = anchors[selectedIndex];
            anchorIndex = selectedIndex + 1;
            var target = element;
            if (['mermaid', 'image', 'fencedCode', 'table'].includes(elementKind)) {
                var wrapper = document.createElement('div');
                wrapper.className = 'muxy-anchor-block muxy-anchor-kind-' + elementKind;
                if (element.parentNode) {
                    element.parentNode.insertBefore(wrapper, element);
                    wrapper.appendChild(element);
                    target = wrapper;
                }
            }
            target.setAttribute('data-muxy-anchor-id', anchor.id);
            target.setAttribute('data-muxy-line-start', String(anchor.startLine));
            target.setAttribute('data-muxy-line-end', String(anchor.endLine));
        }
    }

    function imageCacheKey(image) {
        if (!image) {
            return '';
        }
        return image.getAttribute('src') || image.currentSrc || '';
    }

    function syncImageAttributes(sourceImage, targetImage) {
        if (!sourceImage || !targetImage) {
            return;
        }
        Array.from(targetImage.attributes).forEach(function (attribute) {
            if (!sourceImage.hasAttribute(attribute.name)) {
                targetImage.removeAttribute(attribute.name);
            }
        });
        Array.from(sourceImage.attributes).forEach(function (attribute) {
            if (attribute.name === 'src' && targetImage.getAttribute('src') === attribute.value) {
                return;
            }
            targetImage.setAttribute(attribute.name, attribute.value);
        });
    }

    function preserveExistingImages(markdownRoot, nextRoot) {
        if (!markdownRoot || !nextRoot) {
            return;
        }
        var imagePool = new Map();
        markdownRoot.querySelectorAll('img[src]').forEach(function (image) {
            var key = imageCacheKey(image);
            if (!key) {
                return;
            }
            if (!imagePool.has(key)) {
                imagePool.set(key, []);
            }
            imagePool.get(key).push(image);
        });
        nextRoot.querySelectorAll('img[src]').forEach(function (image) {
            var key = imageCacheKey(image);
            var candidates = key ? imagePool.get(key) : null;
            if (!candidates || !candidates.length) {
                return;
            }
            var existingImage = candidates.shift();
            syncImageAttributes(image, existingImage);
            image.replaceWith(existingImage);
        });
    }

    async function renderMarkdown(content) {
        var anchors = parseSyncAnchors(content);
        if (!_markedConfigured) {
            marked.use({
                walkTokens: function (token) {
                    if (!token || typeof token !== 'object') {
                        return;
                    }
                    if (token.type === 'link') {
                        var safeHref = sanitizeURL(token.href, { allowData: false, allowBlob: false });
                        if (safeHref) {
                            token.href = safeHref;
                        } else {
                            delete token.href;
                        }
                    }
                    if (token.type === 'image') {
                        var safeSrc = sanitizeURL(token.href, { allowData: true, allowBlob: true });
                        if (safeSrc) {
                            token.href = safeSrc;
                        } else {
                            delete token.href;
                        }
                    }
                }
            });
            _markedConfigured = true;
        }
        marked.setOptions({
            breaks: false,
            gfm: true
        });

        var diagramMap = {};
        content = content.replace(/```mermaid\s*\r?\n([\s\S]*?)```/g, function (match, code) {
            var id = 'mermaid-' + Object.keys(diagramMap).length;
            diagramMap[id] = code.trim();
            return '<div class="mermaid" id="' + id + '" data-muxy-mermaid="true"></div>';
        });
        window.__muxyMermaidDiagrams = diagramMap;

        var html = marked.parse(content);
        var markdownRoot = document.getElementById('markdown');
        var nextRoot = document.createElement('div');
        nextRoot.innerHTML = html;
        sanitizeMarkdownDOM(nextRoot);
        normalizeLocalImageSources(nextRoot);
        preserveExistingImages(markdownRoot, nextRoot);

        var fragment = document.createDocumentFragment();
        while (nextRoot.firstChild) {
            fragment.appendChild(nextRoot.firstChild);
        }
        markdownRoot.replaceChildren(fragment);

        assignHeadingIDs(markdownRoot);
        assignAnchorMetadata(markdownRoot, anchors);
        initializeMermaidControls();

        if (Object.keys(diagramMap).length > 0) {
            try {
                var mermaidReady = await ensureMermaidLoaded();
                if (mermaidReady && typeof mermaid !== 'undefined') {
                    var mermaidConfig = {
                        startOnLoad: false,
                        securityLevel: 'strict',
                        theme: window.__muxyMermaidBaseTheme || 'default',
                        flowchart: { htmlLabels: false }
                    };
                    if (window.__muxyMermaidUseThemeVariables && window.__muxyMermaidThemeVariables) {
                        mermaidConfig.themeVariables = window.__muxyMermaidThemeVariables;
                    }
                    mermaid.initialize(mermaidConfig);
                    for (var id in diagramMap) {
                        var el = document.getElementById(id);
                        if (el) {
                            try {
                                var rendered = await mermaid.render(id + '-svg', diagramMap[id]);
                                el.innerHTML = rendered.svg;
                            } catch (err) {
                                el.innerHTML = '<div class="mermaid-error">Diagram Error: '
                                    + escapeHTML(err.message || err)
                                    + '</div>';
                            }
                        }
                    }
                    initializeMermaidControls();
                } else {
                    for (var failedId in diagramMap) {
                        var failedEl = document.getElementById(failedId);
                        if (failedEl) {
                            failedEl.innerHTML = '<div class="mermaid-error">'
                                + 'Mermaid.js not loaded.'
                                + '</div>';
                        }
                    }
                }
            } catch (err) {
                console.error('Mermaid render error:', err);
            }
        }
    }

    window.__muxyRenderMarkdown = function (base64Payload) {
        var markdownPayload = decodeBase64UTF8(String(base64Payload || ''));
        renderMarkdown(markdownPayload).catch(function (err) {
            window.__muxyErrors.push({
                type: 'render-error',
                message: String((err && err.message) ? err.message : err),
                source: 'renderMarkdown'
            });
            console.error('renderMarkdown failed:', err);
        });
        return true;
    };

    window.__muxyRerenderMermaid = async function () {
        var diagrams = window.__muxyMermaidDiagrams;
        if (!diagrams || !Object.keys(diagrams).length) {
            return false;
        }
        try {
            var ready = await ensureMermaidLoaded();
            if (!ready || typeof mermaid === 'undefined') {
                return false;
            }
            var config = {
                startOnLoad: false,
                securityLevel: 'strict',
                theme: window.__muxyMermaidBaseTheme || 'default',
                flowchart: { htmlLabels: false }
            };
            if (window.__muxyMermaidUseThemeVariables && window.__muxyMermaidThemeVariables) {
                config.themeVariables = window.__muxyMermaidThemeVariables;
            }
            mermaid.initialize(config);
            for (var id in diagrams) {
                var el = document.getElementById(id);
                if (!el) continue;
                try {
                    var rendered = await mermaid.render(id + '-svg-' + Date.now(), diagrams[id]);
                    el.innerHTML = rendered.svg;
                } catch (err) {
                    el.innerHTML = '<div class="mermaid-error">Diagram Error: '
                        + escapeHTML(err.message || err) + '</div>';
                }
            }
            initializeMermaidControls();
            return true;
        } catch (_) {
            return false;
        }
    };
})();
