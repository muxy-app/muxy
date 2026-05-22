import AppKit
import Foundation

@MainActor
enum BrowserAnnotationSender {
    static func send(
        annotation: BrowserAnnotation,
        from state: BrowserTabState,
        appState: AppState,
        markSent: () -> Void
    ) {
        let markdown = renderMarkdown(annotation: annotation)
        guard let target = resolveTerminalTarget(state: state, appState: appState) else {
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

    private static func resolveTerminalTarget(state: BrowserTabState, appState: AppState) -> TerminalTarget? {
        let candidateProjectIDs = candidateProjectIDs(for: state, appState: appState)
        for projectID in candidateProjectIDs {
            if let target = findTerminalTarget(projectID: projectID, appState: appState) {
                return target
            }
        }
        return nil
    }

    private static func candidateProjectIDs(for _: BrowserTabState, appState: AppState) -> [UUID] {
        var ordered: [UUID] = []
        if let activeID = appState.activeProjectID {
            ordered.append(activeID)
        }
        for key in appState.workspaceRoots.keys where !ordered.contains(key.projectID) {
            ordered.append(key.projectID)
        }
        return ordered
    }

    private static func findTerminalTarget(projectID: UUID, appState: AppState) -> TerminalTarget? {
        let focusedArea = appState.focusedArea(for: projectID)
        let areas = appState.allAreas(for: projectID)

        if let area = focusedArea,
           let target = preferActiveTerminal(in: area, projectID: projectID)
        {
            return target
        }

        for area in areas {
            if let target = preferActiveTerminal(in: area, projectID: projectID) {
                return target
            }
        }

        for area in areas {
            for tab in area.tabs {
                if let pane = tab.content.pane {
                    return TerminalTarget(projectID: projectID, areaID: area.id, tabID: tab.id, paneID: pane.id)
                }
            }
        }
        return nil
    }

    private static func preferActiveTerminal(in area: TabArea, projectID: UUID) -> TerminalTarget? {
        guard let activeTabID = area.activeTabID,
              let activeTab = area.tabs.first(where: { $0.id == activeTabID }),
              let pane = activeTab.content.pane
        else { return nil }
        return TerminalTarget(projectID: projectID, areaID: area.id, tabID: activeTab.id, paneID: pane.id)
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
        let sanitized = markdown.replacingOccurrences(of: "\u{1B}[201~", with: "")
        var payload = Data()
        payload.append(TerminalControlBytes.bracketedPasteStart)
        payload.append(Data(sanitized.utf8))
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
        lines.append("@muxy-browser: \(annotation.pageURL)")
        if !annotation.pageTitle.isEmpty {
            lines.append("- page: \(annotation.pageTitle)")
        }
        lines.append("- selector: `\(annotation.selector)`")
        if !annotation.xpath.isEmpty {
            lines.append("- xpath: `\(annotation.xpath)`")
        }
        if !annotation.textSnippet.isEmpty {
            lines.append("- text: \"\(annotation.textSnippet)\"")
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
            lines.append("- style override: \(override.property.cssName): \(original) → \(override.value)")
        }
        let comment = annotation.comment.trimmingCharacters(in: .whitespacesAndNewlines)
        if !comment.isEmpty {
            lines.append("- comment: \"\(comment)\"")
        }
        return lines.joined(separator: "\n")
    }
}
