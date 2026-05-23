import AppKit
import Foundation

@MainActor
enum BrowserAnnotationSender {
    enum Target: Equatable {
        case richInput(worktreeKey: WorktreeKey)
        case terminal(TerminalTarget)
    }

    struct TerminalTarget: Equatable {
        let projectID: UUID
        let areaID: UUID
        let tabID: UUID
        let paneID: UUID
    }

    static func send(
        annotation: BrowserAnnotation,
        from session: BrowserSession,
        appState: AppState,
        markSent: () -> Void
    ) {
        send(
            annotation: annotation,
            from: session,
            appState: appState,
            controller: RichInputController.shared,
            markSent: markSent
        )
    }

    static func send(
        annotation: BrowserAnnotation,
        from session: BrowserSession,
        appState: AppState,
        controller: RichInputController,
        markSent: () -> Void
    ) {
        let markdown = renderMarkdown(annotation: annotation)
        guard let target = resolveTarget(session: session, appState: appState, controller: controller) else {
            copyToClipboard(markdown)
            return
        }
        switch target {
        case let .richInput(worktreeKey):
            guard controller.appendMarkdown(markdown, for: worktreeKey) else { return }
            markSent()
        case let .terminal(terminalTarget):
            focus(target: terminalTarget, appState: appState)
            guard let view = TerminalViewRegistry.shared.existingView(for: terminalTarget.paneID) else {
                copyToClipboard(markdown)
                return
            }
            inject(markdown: markdown, into: view)
            markSent()
        }
    }

    static func resolveTarget(
        session: BrowserSession,
        appState: AppState,
        controller: RichInputController
    ) -> Target? {
        if controller.isPanelVisible,
           let activeProjectID = appState.activeProjectID,
           let visibleKey = appState.activeWorktreeKey(for: activeProjectID)
        {
            return .richInput(worktreeKey: visibleKey)
        }
        guard let browserKey = locateOwningWorktreeKey(browserTabID: session.id, appState: appState) else {
            return nil
        }
        if let pane = appState.lastActiveTerminalPane(for: browserKey) {
            return .terminal(TerminalTarget(
                projectID: browserKey.projectID,
                areaID: pane.areaID,
                tabID: pane.tabID,
                paneID: pane.paneID
            ))
        }
        if let terminal = firstTerminalInWorktree(
            browserTabID: session.id,
            worktreeKey: browserKey,
            appState: appState
        ) {
            return .terminal(terminal)
        }
        return nil
    }

    static func locateOwningWorktreeKey(browserTabID: UUID, appState: AppState) -> WorktreeKey? {
        for (key, root) in appState.workspaceRoots where root.allAreas().contains(where: { area in
            area.tabs.contains(where: { $0.id == browserTabID })
        }) {
            return key
        }
        return nil
    }

    private static func firstTerminalInWorktree(
        browserTabID: UUID,
        worktreeKey: WorktreeKey,
        appState: AppState
    ) -> TerminalTarget? {
        guard let root = appState.workspaceRoots[worktreeKey] else { return nil }
        let areas = root.allAreas()
        if let owningArea = areas.first(where: { area in
            area.tabs.contains(where: { $0.id == browserTabID })
        }),
            let target = firstTerminal(in: owningArea, projectID: worktreeKey.projectID)
        {
            return target
        }
        for area in areas where !area.tabs.contains(where: { $0.id == browserTabID }) {
            if let target = firstTerminal(in: area, projectID: worktreeKey.projectID) {
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
        appState.dispatch(.selectTab(
            projectID: target.projectID,
            areaID: target.areaID,
            tabID: target.tabID
        ))
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
