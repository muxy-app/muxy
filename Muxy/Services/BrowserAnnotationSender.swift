import AppKit
import Foundation

@MainActor
enum BrowserAnnotationSender {
    static func send(
        annotation: BrowserAnnotation,
        from session: BrowserSession,
        appState: AppState,
        markSent: () -> Void
    ) {
        let markdown = renderMarkdown(annotation: annotation)
        guard let target = resolveTerminalTarget(session: session, appState: appState) else {
            copyToClipboard(markdown)
            return
        }
        focus(target: target, appState: appState)
        guard let view = TerminalViewRegistry.shared.existingView(for: target.paneID) else {
            copyToClipboard(markdown)
            return
        }
        inject(markdown: markdown, into: view)
        markSent()
    }

    private struct TerminalTarget {
        let projectID: UUID
        let areaID: UUID
        let tabID: UUID
        let paneID: UUID
    }

    private static func resolveTerminalTarget(session: BrowserSession, appState: AppState) -> TerminalTarget? {
        if let target = resolveFromOwningArea(browserTabID: session.id, appState: appState) {
            return target
        }
        return resolveFromActiveProject(appState: appState)
    }

    private static func resolveFromOwningArea(browserTabID: UUID, appState: AppState) -> TerminalTarget? {
        for (key, root) in appState.workspaceRoots {
            let areas = root.allAreas()
            guard let owningArea = areas.first(where: { area in
                area.tabs.contains(where: { $0.id == browserTabID })
            })
            else { continue }
            if let target = firstTerminal(in: owningArea, projectID: key.projectID) {
                return target
            }
            for sibling in areas where sibling.id != owningArea.id {
                if let target = firstTerminal(in: sibling, projectID: key.projectID) {
                    return target
                }
            }
        }
        return nil
    }

    private static func resolveFromActiveProject(appState: AppState) -> TerminalTarget? {
        guard let projectID = appState.activeProjectID else { return nil }
        let areas = appState.allAreas(for: projectID)
        if let focused = appState.focusedArea(for: projectID),
           let target = firstTerminal(in: focused, projectID: projectID)
        {
            return target
        }
        for area in areas {
            if let target = firstTerminal(in: area, projectID: projectID) {
                return target
            }
        }
        return nil
    }

    private static func firstTerminal(in area: TabArea, projectID: UUID) -> TerminalTarget? {
        if let activeTabID = area.activeTabID,
           let activeTab = area.tabs.first(where: { $0.id == activeTabID }),
           let pane = activeTab.content.pane
        {
            return TerminalTarget(projectID: projectID, areaID: area.id, tabID: activeTab.id, paneID: pane.id)
        }
        for tab in area.tabs {
            if let pane = tab.content.pane {
                return TerminalTarget(projectID: projectID, areaID: area.id, tabID: tab.id, paneID: pane.id)
            }
        }
        return nil
    }

    private static func focus(target: TerminalTarget, appState: AppState) {
        guard let key = appState.activeWorktreeKey(for: target.projectID),
              let area = appState.workspaceRoots[key]?.findArea(id: target.areaID)
        else { return }
        if area.activeTabID != target.tabID {
            area.selectTab(target.tabID)
        }
    }

    private static func inject(markdown: String, into view: GhosttyTerminalNSView) {
        var payload = Data()
        payload.append(TerminalControlBytes.bracketedPasteStart)
        payload.append(Data(markdown.utf8))
        payload.append(TerminalControlBytes.bracketedPasteEnd)
        view.sendRemoteBytes(payload)
        view.window?.makeFirstResponder(view)
    }

    private static func copyToClipboard(_ markdown: String) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(markdown, forType: .string)
        ToastState.shared.show("No terminal pane found — annotation copied to clipboard")
    }

    static func renderMarkdown(annotation: BrowserAnnotation) -> String {
        var lines: [String] = []
        let url = BrowserAnnotationSanitizer.sanitizeURLString(annotation.pageURL)
        lines.append("@muxy-browser: \(url)")
        let title = BrowserAnnotationSanitizer.sanitizeSingleLine(
            annotation.pageTitle,
            maxLength: BrowserAnnotationSanitizer.maxTitleLength
        )
        if !title.isEmpty {
            lines.append("- page: \(title)")
        }
        let selector = BrowserAnnotationSanitizer.sanitizeMarkdownInlineCode(
            annotation.selector,
            maxLength: BrowserAnnotationSanitizer.maxSelectorLength
        )
        lines.append("- selector: `\(selector)`")
        let xpath = BrowserAnnotationSanitizer.sanitizeMarkdownInlineCode(
            annotation.xpath,
            maxLength: BrowserAnnotationSanitizer.maxXPathLength
        )
        if !xpath.isEmpty {
            lines.append("- xpath: `\(xpath)`")
        }
        let snippet = BrowserAnnotationSanitizer.sanitizeSingleLine(
            annotation.textSnippet,
            maxLength: BrowserAnnotationSanitizer.maxTextSnippetLength
        )
        if !snippet.isEmpty {
            lines.append("- text: \"\(snippet)\"")
        }
        let bbox = String(
            format: "(x=%.0f, y=%.0f, w=%.0f, h=%.0f)",
            annotation.rect.origin.x,
            annotation.rect.origin.y,
            annotation.rect.width,
            annotation.rect.height
        )
        lines.append("- bbox: \(bbox)")
        let viewport = String(format: "%.0f×%.0f", annotation.viewportWidth, annotation.viewportHeight)
        lines.append("- viewport: \(viewport)")
        for override in annotation.styleOverrides {
            let original = override.originalValue.isEmpty ? "default" : override.originalValue
            let sanitizedOriginal = BrowserAnnotationSanitizer.sanitizeSingleLine(
                original,
                maxLength: BrowserAnnotationSanitizer.maxStyleValueLength
            )
            let sanitizedValue = BrowserAnnotationSanitizer.sanitizeSingleLine(
                override.value,
                maxLength: BrowserAnnotationSanitizer.maxStyleValueLength
            )
            lines.append("- style override: \(override.property.cssName): \(sanitizedOriginal) → \(sanitizedValue)")
        }
        let comment = BrowserAnnotationSanitizer
            .sanitizeMultiLine(annotation.comment, maxLength: BrowserAnnotationSanitizer.maxCommentLength)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if !comment.isEmpty {
            lines.append("- comment: \"\(comment)\"")
        }
        return lines.joined(separator: "\n")
    }
}
