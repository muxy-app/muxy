import AppKit
import Foundation
import os

private let markdownLogger = Logger(subsystem: "app.muxy", category: "MarkdownPreview")

private struct MermaidThemeVariables: Codable {
    let primaryColor: String
    let primaryTextColor: String
    let primaryBorderColor: String
    let textColor: String
    let lineColor: String
    let secondaryColor: String
    let tertiaryColor: String
    let background: String
    let mainBkg: String
    let secondBkg: String
    let tertiaryBkg: String
    let nodeBorder: String
    let clusterBkg: String
    let clusterBorder: String
    let defaultLinkColor: String
    let titleColor: String
    let edgeLabelBackground: String
    let nodeTextColor: String
    let labelTextColor: String
    let noteBkgColor: String
    let noteTextColor: String
    let noteBorderColor: String
    let actorBkg: String
    let actorBorder: String
    let actorTextColor: String
    let actorLineColor: String
    let signalColor: String
    let signalTextColor: String
    let labelBoxBkgColor: String
    let labelBoxBorderColor: String
    let loopTextColor: String
    let activationBorderColor: String
    let activationBkgColor: String
    let sequenceNumberColor: String
    let classText: String
    let entityBkgColor: String
    let entityBorderColor: String
    let entityTextColor: String
    let sectionBkgColor: String
    let altSectionBkgColor: String
    let sectionBkgColor2: String
    let taskBkgColor: String
    let taskTextColor: String
    let taskTextDarkColor: String
    let taskTextOutsideColor: String
    let taskTextClickableColor: String
    let activeTaskBkgColor: String
    let doneTaskBkgColor: String
    let doneTaskBorderColor: String
    let critBorderColor: String
    let critBkgColor: String
    let todayLineColor: String
    let personBorder: String
    let personBkg: String
    let pie1: String
    let pie2: String
    let pie3: String
    let pie4: String
    let pie5: String
    let pie6: String
    let pie7: String
    let pie8: String
    let pie9: String
    let pie10: String
    let pie11: String
    let pie12: String

    var jsObjectLiteral: String {
        guard let data = try? JSONEncoder().encode(self),
              let json = String(data: data, encoding: .utf8)
        else {
            return "{}"
        }
        return json
    }
}

enum MarkdownRenderer {
    struct Palette: Equatable {
        let background: NSColor
        let foreground: NSColor
        let accent: NSColor
    }

    @MainActor
    static func html(filePath: String?) -> String {
        let title = escapeForHTML(filePath.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Markdown")
        let imageBaseHost = filePath.flatMap { encodedImageBaseHost(forMarkdownFilePath: $0) } ?? ""
        return """
        <!DOCTYPE html>
        <html>
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>\(title)</title>
            <script src="muxy-asset://markdown/marked.min.js"></script>
            <style id="muxy-base-style">
                :root {
                    color-scheme: light;
                    --fg: #1F2328;
                    --accent: #0969DA;
                    --border: #D0D7DE;
                    --muted: #57606A;
                    --code-bg: #F6F8FA;
                    --blockquote-border: #D0D7DE;
                    --row-alt: #F6F8FA;
                }
                * { box-sizing: border-box; margin: 0; padding: 0; }
                html, body {
                    background: transparent;
                    color: var(--fg);
                    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
                    font-size: 14px;
                    line-height: 1.6;
                    padding: 0;
                    margin: 0;
                    height: 100%;
                    overflow: hidden;
                }
                #content {
                    height: 100%;
                    overflow-y: auto;
                    padding: 24px 32px max(60px, 40vh) 32px;
                    box-sizing: border-box;
                }
                .markdown-body {
                    max-width: 900px;
                    margin: 0 auto;
                    color: var(--fg);
                }
                .muxy-anchor-block {
                    display: block;
                    position: relative;
                }
                .markdown-body h1, .markdown-body h2, .markdown-body h3,
                .markdown-body h4, .markdown-body h5, .markdown-body h6 {
                    color: var(--fg);
                    font-weight: 600;
                    margin-top: 24px;
                    margin-bottom: 16px;
                    line-height: 1.25;
                }
                .markdown-body h1 { font-size: 2em; border-bottom: 1px solid var(--border); padding-bottom: 0.3em; }
                .markdown-body h2 { font-size: 1.5em; border-bottom: 1px solid var(--border); padding-bottom: 0.3em; }
                .markdown-body h3 { font-size: 1.25em; }
                .markdown-body h4 { font-size: 1em; }
                .markdown-body a { color: var(--accent); text-decoration: none; }
                .markdown-body a:hover { text-decoration: underline; }
                .markdown-body code {
                    background: var(--code-bg);
                    border-radius: 4px;
                    padding: 0.2em 0.4em;
                    font-size: 85%;
                    font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
                }
                .markdown-body pre {
                    background: var(--code-bg);
                    border: 1px solid var(--border);
                    border-radius: 6px;
                    padding: 16px;
                    overflow: auto;
                    margin: 16px 0;
                }
                .markdown-body pre code {
                    background: transparent;
                    padding: 0;
                    font-size: 90%;
                    border-radius: 0;
                    white-space: pre;
                }
                .markdown-body blockquote {
                    border-left: 4px solid var(--blockquote-border);
                    padding: 0 16px;
                    color: var(--muted);
                    margin: 16px 0;
                }
                .markdown-body table {
                    border-collapse: collapse;
                    width: 100%;
                    margin: 16px 0;
                }
                .markdown-body table th, .markdown-body table td {
                    border: 1px solid var(--border);
                    padding: 8px 13px;
                    text-align: left;
                }
                .markdown-body table th { background: var(--code-bg); font-weight: 600; }
                .markdown-body table tr:nth-child(even) { background: var(--row-alt); }
                .markdown-body img { max-width: 100%; border-radius: 4px; }
                .markdown-body ul, .markdown-body ol { padding-left: 2em; margin: 16px 0; }
                .markdown-body li { margin: 4px 0; }
                .markdown-body hr { border: none; border-top: 1px solid var(--border); margin: 24px 0; }
                .mermaid {
                    width: 100%;
                    margin: 16px 0;
                }
                .mermaid .mermaid-toolbar {
                    display: flex;
                    align-items: center;
                    gap: 6px;
                    justify-content: flex-end;
                    margin-bottom: 6px;
                    font-size: 11px;
                    color: var(--muted);
                }
                .mermaid .mermaid-btn {
                    border: 1px solid var(--border);
                    background: var(--code-bg);
                    color: var(--fg);
                    border-radius: 4px;
                    padding: 2px 6px;
                    cursor: pointer;
                    font-size: 11px;
                    line-height: 1.2;
                }
                .mermaid .mermaid-btn:hover {
                    border-color: var(--accent);
                }
                .mermaid .mermaid-zoom-label {
                    min-width: 42px;
                    text-align: center;
                    color: var(--muted);
                }
                .mermaid .mermaid-canvas {
                    width: 100%;
                    overflow: auto;
                }
                .mermaid svg {
                    display: block;
                    max-width: 100%;
                    width: 100%;
                    height: auto;
                    margin: 0 auto;
                }
                .mermaid[data-size-mode="natural"] svg {
                    max-width: none;
                    width: auto;
                }
                .mermaid svg[width],
                .mermaid svg[height] {
                    max-width: 100%;
                    height: auto;
                }
                .mermaid-error {
                    background: rgba(248, 81, 73, 0.1);
                    border: 1px solid rgba(248, 81, 73, 0.3);
                    border-radius: 6px;
                    padding: 12px 16px;
                    color: #f85149;
                    font-size: 13px;
                    margin: 16px 0;
                }
                .markdown-body pre.muxy-prehl { background: var(--code-bg); }
                .markdown-body pre.muxy-prehl code.muxy-hl { color: var(--fg); }
            </style>
            <style id="muxy-syntax-style"></style>
        </head>
        <body>
            <div id="content">
                <div id="markdown" class="markdown-body"></div>
            </div>
            <script>
                window.__muxyImageBaseHost = "\(imageBaseHost)";
            </script>
            <script src="muxy-asset://markdown/markdown-renderer.js"></script>
        </body>
        </html>
        """
    }

    static func updateScript(content: String) -> String {
        let preparedMermaidContent = MermaidCodeBlockNormalizer.normalizeMermaidCodeBlocks(in: content)
        let preparedContent = MarkdownCodeBlockHighlighter.prerenderCodeBlocks(in: preparedMermaidContent)
        let encodedPayload = Data(preparedContent.utf8).base64EncodedString()
        return """
        (() => {
            if (typeof window.__muxyRenderMarkdown !== 'function') {
                return false;
            }
            return window.__muxyRenderMarkdown("\(encodedPayload)");
        })();
        """
    }

    @MainActor
    static func themeApplyScript(palette: Palette) -> String {
        let bgHex = colorToHex(palette.background)
        let fgHex = colorToHex(palette.foreground)
        let accentHex = colorToHex(palette.accent)
        let borderHex = colorToHex(blend(foreground: palette.foreground, background: palette.background, amount: 0.2))
        let mutedHex = colorToHex(blend(foreground: palette.foreground, background: palette.background, amount: 0.65))
        let codeBgHex = colorToHex(blend(foreground: palette.foreground, background: palette.background, amount: 0.08))
        let rowAltHex = colorToHex(blend(foreground: palette.foreground, background: palette.background, amount: 0.04))
        let mermaidSecondaryHex = colorToHex(blend(foreground: palette.foreground, background: palette.background, amount: 0.12))
        let mermaidTertiaryHex = colorToHex(blend(foreground: palette.foreground, background: palette.background, amount: 0.18))
        let accentSoftHex = colorToHex(blend(foreground: palette.accent, background: palette.background, amount: 0.22))
        let accentSubtleHex = colorToHex(blend(foreground: palette.accent, background: palette.background, amount: 0.12))
        let accentMutedHex = colorToHex(blend(foreground: palette.accent, background: palette.background, amount: 0.35))
        let accentStrongHex = colorToHex(blend(foreground: palette.accent, background: palette.background, amount: 0.5))
        let mermaidThemeVariables = MermaidThemeVariables(
            primaryColor: "#\(accentHex)",
            primaryTextColor: "#\(fgHex)",
            primaryBorderColor: "#\(borderHex)",
            textColor: "#\(fgHex)",
            lineColor: "#\(mutedHex)",
            secondaryColor: "#\(mermaidSecondaryHex)",
            tertiaryColor: "#\(mermaidTertiaryHex)",
            background: "#\(bgHex)",
            mainBkg: "#\(codeBgHex)",
            secondBkg: "#\(mermaidSecondaryHex)",
            tertiaryBkg: "#\(mermaidTertiaryHex)",
            nodeBorder: "#\(borderHex)",
            clusterBkg: "#\(mermaidSecondaryHex)",
            clusterBorder: "#\(borderHex)",
            defaultLinkColor: "#\(mutedHex)",
            titleColor: "#\(fgHex)",
            edgeLabelBackground: "#\(codeBgHex)",
            nodeTextColor: "#\(fgHex)",
            labelTextColor: "#\(fgHex)",
            noteBkgColor: "#\(codeBgHex)",
            noteTextColor: "#\(fgHex)",
            noteBorderColor: "#\(borderHex)",
            actorBkg: "#\(mermaidSecondaryHex)",
            actorBorder: "#\(borderHex)",
            actorTextColor: "#\(fgHex)",
            actorLineColor: "#\(mutedHex)",
            signalColor: "#\(fgHex)",
            signalTextColor: "#\(fgHex)",
            labelBoxBkgColor: "#\(codeBgHex)",
            labelBoxBorderColor: "#\(borderHex)",
            loopTextColor: "#\(fgHex)",
            activationBorderColor: "#\(borderHex)",
            activationBkgColor: "#\(accentSubtleHex)",
            sequenceNumberColor: "#\(bgHex)",
            classText: "#\(fgHex)",
            entityBkgColor: "#\(codeBgHex)",
            entityBorderColor: "#\(borderHex)",
            entityTextColor: "#\(fgHex)",
            sectionBkgColor: "#\(mermaidSecondaryHex)",
            altSectionBkgColor: "#\(codeBgHex)",
            sectionBkgColor2: "#\(mermaidTertiaryHex)",
            taskBkgColor: "#\(accentSoftHex)",
            taskTextColor: "#\(fgHex)",
            taskTextDarkColor: "#\(bgHex)",
            taskTextOutsideColor: "#\(fgHex)",
            taskTextClickableColor: "#\(accentHex)",
            activeTaskBkgColor: "#\(accentMutedHex)",
            doneTaskBkgColor: "#\(mermaidTertiaryHex)",
            doneTaskBorderColor: "#\(borderHex)",
            critBorderColor: "#\(accentStrongHex)",
            critBkgColor: "#\(accentMutedHex)",
            todayLineColor: "#\(accentHex)",
            personBorder: "#\(borderHex)",
            personBkg: "#\(mermaidSecondaryHex)",
            pie1: "#\(accentHex)",
            pie2: "#\(accentMutedHex)",
            pie3: "#\(mermaidSecondaryHex)",
            pie4: "#\(mermaidTertiaryHex)",
            pie5: "#\(accentSoftHex)",
            pie6: "#\(accentStrongHex)",
            pie7: "#\(borderHex)",
            pie8: "#\(mutedHex)",
            pie9: "#\(codeBgHex)",
            pie10: "#\(accentSubtleHex)",
            pie11: "#\(mermaidSecondaryHex)",
            pie12: "#\(mermaidTertiaryHex)"
        )
        let mermaidThemeJSON = mermaidThemeVariables.jsObjectLiteral
        let isDarkPreview = isDarkColor(palette.background)
        let mermaidBaseTheme = isDarkPreview ? "dark" : "default"
        let colorScheme = isDarkPreview ? "dark" : "light"
        let codeBackground = blend(foreground: palette.foreground, background: palette.background, amount: 0.08)
        let syntaxCSS = SyntaxHTMLRenderer.cssStylesheet(background: codeBackground, foreground: palette.foreground)
        let escapedSyntaxCSS = syntaxCSS
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "`", with: "\\`")
            .replacingOccurrences(of: "$", with: "\\$")
        return """
        (() => {
            const root = document.documentElement;
            if (!root) return;
            root.style.colorScheme = "\(colorScheme)";
            root.style.setProperty('--bg', '#\(bgHex)');
            root.style.setProperty('--fg', '#\(fgHex)');
            root.style.setProperty('--accent', '#\(accentHex)');
            root.style.setProperty('--border', '#\(borderHex)');
            root.style.setProperty('--muted', '#\(mutedHex)');
            root.style.setProperty('--code-bg', '#\(codeBgHex)');
            root.style.setProperty('--blockquote-border', '#\(borderHex)');
            root.style.setProperty('--row-alt', '#\(rowAltHex)');
            const syntaxStyle = document.getElementById('muxy-syntax-style');
            if (syntaxStyle) {
                syntaxStyle.textContent = `\(escapedSyntaxCSS)`;
            }
            window.__muxyMermaidBaseTheme = "\(mermaidBaseTheme)";
            window.__muxyMermaidUseThemeVariables = \(isDarkPreview ? "true" : "false");
            window.__muxyMermaidThemeVariables = \(mermaidThemeJSON);
            if (typeof window.__muxyRerenderMermaid === 'function') {
                window.__muxyRerenderMermaid();
            }
        })();
        """
    }

    private static func colorToHex(_ color: NSColor) -> String {
        let colorSpaces: [NSColorSpace] = [.sRGB, .extendedSRGB, .deviceRGB, .genericRGB]
        for colorSpace in colorSpaces {
            if let rgb = color.usingColorSpace(colorSpace) {
                let r = Int(round(rgb.redComponent * 255))
                let g = Int(round(rgb.greenComponent * 255))
                let b = Int(round(rgb.blueComponent * 255))
                return String(format: "%02X%02X%02X", max(0, min(255, r)), max(0, min(255, g)), max(0, min(255, b)))
            }
        }

        markdownLogger.error("Failed to convert NSColor to RGB hex, using fallback")
        return "1E1E1E"
    }

    private static func encodedImageBaseHost(forMarkdownFilePath path: String) -> String? {
        let directory = URL(fileURLWithPath: path).deletingLastPathComponent().standardizedFileURL.path
        guard let data = directory.data(using: .utf8) else { return nil }
        return data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    private static func isDarkColor(_ color: NSColor) -> Bool {
        guard let rgb = color.usingColorSpace(.sRGB) else { return false }
        let luminance = 0.2126 * rgb.redComponent + 0.7152 * rgb.greenComponent + 0.0722 * rgb.blueComponent
        return luminance < 0.5
    }

    private static func blend(foreground: NSColor, background: NSColor, amount: CGFloat) -> NSColor {
        let a = max(0, min(1, amount))
        guard let fg = foreground.usingColorSpace(.sRGB),
              let bg = background.usingColorSpace(.sRGB)
        else {
            return foreground.withAlphaComponent(a)
        }

        let r = bg.redComponent + (fg.redComponent - bg.redComponent) * a
        let g = bg.greenComponent + (fg.greenComponent - bg.greenComponent) * a
        let b = bg.blueComponent + (fg.blueComponent - bg.blueComponent) * a
        return NSColor(srgbRed: r, green: g, blue: b, alpha: 1)
    }

    private static func escapeForHTML(_ string: String) -> String {
        string
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "'", with: "&#39;")
    }
}
