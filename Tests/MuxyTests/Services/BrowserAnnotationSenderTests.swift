import Foundation
import Testing

@testable import Muxy

@Suite("BrowserAnnotationSender")
@MainActor
struct BrowserAnnotationSenderTests {
    @Test("renders markdown with selector, viewport, and comment")
    func rendersMarkdown() {
        let annotation = BrowserAnnotation(
            selector: "main > header h1.title",
            xpath: "/html/body/main/header/h1",
            textSnippet: "Welcome",
            rect: CGRect(x: 10, y: 20, width: 120, height: 40),
            pageURL: "https://example.com/landing",
            pageTitle: "Example",
            viewportWidth: 1440,
            viewportHeight: 900,
            comment: "Make this larger"
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)

        #expect(markdown.contains("@muxy-browser: https://example.com/landing"))
        #expect(markdown.contains("- page: Example"))
        #expect(markdown.contains("- selector: `main > header h1.title`"))
        #expect(markdown.contains("- xpath: `/html/body/main/header/h1`"))
        #expect(markdown.contains("- text: \"Welcome\""))
        #expect(markdown.contains("- bbox: (x=10, y=20, w=120, h=40)"))
        #expect(markdown.contains("- viewport: 1440×900"))
        #expect(markdown.contains("- comment: \"Make this larger\""))
    }

    @Test("includes style override lines")
    func includesStyleOverrides() {
        let override = StyleOverride(
            selector: ".btn",
            property: .backgroundColor,
            originalValue: "rgb(0, 0, 0)",
            value: "#fff"
        )
        let annotation = BrowserAnnotation(
            selector: ".btn",
            xpath: "",
            textSnippet: "Submit",
            rect: .zero,
            pageURL: "https://example.com",
            pageTitle: "",
            viewportWidth: 800,
            viewportHeight: 600,
            styleOverrides: [override]
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)
        #expect(markdown.contains("- style override: background-color: rgb(0, 0, 0) → #fff"))
    }

    @Test("strips terminal control sequences from page-supplied fields")
    func stripsTerminalControlSequences() {
        let annotation = BrowserAnnotation(
            selector: "a\u{1B}[2K",
            xpath: "x\u{07}",
            textSnippet: "evil\u{1B}[201~payload",
            rect: .zero,
            pageURL: "https://example.com\u{1B}",
            pageTitle: "title\u{07}",
            viewportWidth: 0,
            viewportHeight: 0,
            comment: "comment\u{1B}[31m\nstill ok"
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)

        #expect(!markdown.contains("\u{1B}"))
        #expect(!markdown.contains("\u{07}"))
        #expect(!markdown.contains("\u{7F}"))
    }

    @Test("escapes backticks in inline code so a page cannot break out of code spans")
    func escapesBackticksInInlineCode() {
        let annotation = BrowserAnnotation(
            selector: "main`; rm -rf /; echo `",
            xpath: "/html/body`evil`",
            textSnippet: "",
            rect: .zero,
            pageURL: "https://example.com",
            pageTitle: "",
            viewportWidth: 0,
            viewportHeight: 0
        )

        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)

        let selectorLine = markdown
            .split(whereSeparator: \.isNewline)
            .first(where: { $0.hasPrefix("- selector: ") }) ?? ""
        let xpathLine = markdown
            .split(whereSeparator: \.isNewline)
            .first(where: { $0.hasPrefix("- xpath: ") }) ?? ""
        #expect(selectorLine.filter { $0 == "`" }.count == 2)
        #expect(xpathLine.filter { $0 == "`" }.count == 2)
    }

    @Test("caps oversized comment input")
    func capsOversizedComment() {
        let annotation = BrowserAnnotation(
            selector: ".x",
            xpath: "",
            textSnippet: "",
            rect: .zero,
            pageURL: "https://example.com",
            pageTitle: "",
            viewportWidth: 0,
            viewportHeight: 0,
            comment: String(repeating: "a", count: BrowserAnnotationSanitizer.maxCommentLength + 256)
        )
        let markdown = BrowserAnnotationSender.renderMarkdown(annotation: annotation)
        #expect(markdown.count < BrowserAnnotationSanitizer.maxCommentLength + 1024)
    }

    @Test("routes to the active worktree's rich input when the panel is visible")
    func routesToRichInputWhenPanelVisible() {
        let harness = RoutingHarness.makeWithTerminalAndBrowser()
        harness.controller.isPanelVisible = true

        let target = BrowserAnnotationSender.resolveTarget(
            session: harness.browserSession,
            appState: harness.appState,
            controller: harness.controller
        )

        #expect(target == .richInput(worktreeKey: harness.key))
    }

    @Test("routes to last active terminal pane when rich input is hidden")
    func routesToLastActiveTerminalWhenRichInputHidden() {
        let harness = RoutingHarness.makeWithTerminalAndBrowser()

        let target = BrowserAnnotationSender.resolveTarget(
            session: harness.browserSession,
            appState: harness.appState,
            controller: harness.controller
        )

        guard case let .terminal(terminal) = target else {
            Issue.record("Expected terminal target, got \(String(describing: target))")
            return
        }
        #expect(terminal.paneID == harness.terminalPaneID)
        #expect(terminal.areaID == harness.area.id)
        #expect(terminal.tabID == harness.terminalTabID)
    }

    @Test("falls back to owning area when no last active terminal is tracked")
    func fallsBackToOwningAreaWhenNoLastActive() {
        let harness = RoutingHarness.makeWithTerminalAndBrowser()
        harness.appState.lastActiveTerminalPaneID.removeValue(forKey: harness.key)

        let target = BrowserAnnotationSender.resolveTarget(
            session: harness.browserSession,
            appState: harness.appState,
            controller: harness.controller
        )

        guard case let .terminal(terminal) = target else {
            Issue.record("Expected terminal fallback, got \(String(describing: target))")
            return
        }
        #expect(terminal.paneID == harness.terminalPaneID)
        #expect(terminal.areaID == harness.area.id)
    }

    @Test("rich input takes precedence over last active terminal when panel is visible")
    func richInputBeatsLastActiveTerminal() {
        let harness = RoutingHarness.makeWithTerminalAndBrowser()
        harness.controller.isPanelVisible = true

        let target = BrowserAnnotationSender.resolveTarget(
            session: harness.browserSession,
            appState: harness.appState,
            controller: harness.controller
        )

        #expect(target == .richInput(worktreeKey: harness.key))
    }

    @Test("rich input panel visible with no active project falls through to terminal routing")
    func richInputWithoutActiveProjectFallsThroughToTerminal() {
        let harness = RoutingHarness.makeWithTerminalAndBrowser()
        harness.controller.isPanelVisible = true
        harness.appState.activeProjectID = nil

        let target = BrowserAnnotationSender.resolveTarget(
            session: harness.browserSession,
            appState: harness.appState,
            controller: harness.controller
        )

        guard case let .terminal(terminal) = target else {
            Issue.record("Expected terminal fallback when no active project is set")
            return
        }
        #expect(terminal.paneID == harness.terminalPaneID)
    }

    @Test("orphaned browser session resolves to nil so the caller copies to clipboard")
    func orphanedBrowserSessionResolvesToNil() {
        let harness = RoutingHarness.makeWithTerminalAndBrowser()
        let orphan = BrowserSession(projectPath: "/tmp/test", initialURL: "https://orphan")

        let target = BrowserAnnotationSender.resolveTarget(
            session: orphan,
            appState: harness.appState,
            controller: harness.controller
        )

        #expect(target == nil)
    }

    @Test("panel visible routes to the active worktree even if browser session lives in another worktree")
    func panelVisibleRoutesToActiveWorktreeNotBrowserWorktree() {
        let harness = RoutingHarness.makeWithTwoWorktrees()
        harness.controller.isPanelVisible = true

        let target = BrowserAnnotationSender.resolveTarget(
            session: harness.otherBrowserSession,
            appState: harness.appState,
            controller: harness.controller
        )

        #expect(target == .richInput(worktreeKey: harness.activeKey))
    }

    @Test("send updates AppState focus to the terminal area when routing to a terminal")
    func sendUpdatesAppStateFocusToTerminalArea() {
        let harness = RoutingHarness.makeWithTwoAreasInSameWorktree()
        let annotation = BrowserAnnotation(
            selector: ".x",
            xpath: "",
            textSnippet: "",
            rect: .zero,
            pageURL: "https://example.com",
            pageTitle: "",
            viewportWidth: 0,
            viewportHeight: 0
        )
        #expect(harness.appState.focusedAreaID[harness.key] == harness.browserAreaID)

        BrowserAnnotationSender.send(
            annotation: annotation,
            from: harness.browserSession,
            appState: harness.appState,
            controller: harness.controller,
            markSent: {}
        )

        #expect(harness.appState.focusedAreaID[harness.key] == harness.terminalAreaID)
    }

}

@MainActor
private struct RoutingHarness {
    let appState: AppState
    let controller: RichInputController
    let key: WorktreeKey
    let area: TabArea
    let terminalPaneID: UUID
    let terminalTabID: UUID
    let browserSession: BrowserSession
    var activeKey: WorktreeKey { key }
    var otherBrowserSession: BrowserSession { browserSession }
    var browserAreaID: UUID { area.id }
    var terminalAreaID: UUID { terminalAreaIDOverride ?? area.id }

    static func makeWithTerminalAndBrowser() -> RoutingHarness {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: "/tmp/test")
        guard let terminalTab = area.activeTab,
              let terminalPane = terminalTab.content.pane
        else {
            fatalError("TabArea init is expected to seed an initial terminal tab")
        }
        let browserSession = BrowserSession(
            projectPath: "/tmp/test",
            initialURL: "https://example.com"
        )
        area.insertExistingTab(TerminalTab(browserSession: browserSession))

        let appState = AppState(
            selectionStore: RoutingSelectionStoreStub(),
            terminalViews: RoutingTerminalViewRemovingStub(),
            workspacePersistence: RoutingWorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        appState.lastActiveTerminalPaneID[key] = terminalPane.id

        return RoutingHarness(
            appState: appState,
            controller: RichInputController(),
            key: key,
            area: area,
            terminalPaneID: terminalPane.id,
            terminalTabID: terminalTab.id,
            browserSession: browserSession
        )
    }

    static func makeWithTwoWorktrees() -> RoutingHarness {
        let projectID = UUID()
        let activeWorktreeID = UUID()
        let otherWorktreeID = UUID()
        let activeKey = WorktreeKey(projectID: projectID, worktreeID: activeWorktreeID)
        let otherKey = WorktreeKey(projectID: projectID, worktreeID: otherWorktreeID)
        let activeArea = TabArea(projectPath: "/tmp/test")
        let otherArea = TabArea(projectPath: "/tmp/test")
        guard let activeTerminalTab = activeArea.activeTab,
              let activeTerminalPane = activeTerminalTab.content.pane
        else {
            fatalError("TabArea init is expected to seed an initial terminal tab")
        }
        let otherBrowserSession = BrowserSession(
            projectPath: "/tmp/test",
            initialURL: "https://other-worktree"
        )
        otherArea.insertExistingTab(TerminalTab(browserSession: otherBrowserSession))

        let appState = AppState(
            selectionStore: RoutingSelectionStoreStub(),
            terminalViews: RoutingTerminalViewRemovingStub(),
            workspacePersistence: RoutingWorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = activeWorktreeID
        appState.workspaceRoots[activeKey] = .tabArea(activeArea)
        appState.workspaceRoots[otherKey] = .tabArea(otherArea)
        appState.focusedAreaID[activeKey] = activeArea.id
        appState.focusedAreaID[otherKey] = otherArea.id
        appState.lastActiveTerminalPaneID[activeKey] = activeTerminalPane.id

        return RoutingHarness(
            appState: appState,
            controller: RichInputController(),
            key: activeKey,
            area: activeArea,
            terminalPaneID: activeTerminalPane.id,
            terminalTabID: activeTerminalTab.id,
            browserSession: otherBrowserSession
        )
    }

    static func makeWithTwoAreasInSameWorktree() -> RoutingHarness {
        let projectID = UUID()
        let worktreeID = UUID()
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let terminalArea = TabArea(projectPath: "/tmp/test")
        let browserArea = TabArea(projectPath: "/tmp/test")
        guard let terminalTab = terminalArea.activeTab,
              let terminalPane = terminalTab.content.pane
        else {
            fatalError("TabArea init is expected to seed an initial terminal tab")
        }
        let browserSession = BrowserSession(
            projectPath: "/tmp/test",
            initialURL: "https://example.com"
        )
        browserArea.insertExistingTab(TerminalTab(browserSession: browserSession))

        let root = SplitNode.split(SplitBranch(
            direction: .horizontal,
            ratio: 0.5,
            first: .tabArea(terminalArea),
            second: .tabArea(browserArea)
        ))

        let appState = AppState(
            selectionStore: RoutingSelectionStoreStub(),
            terminalViews: RoutingTerminalViewRemovingStub(),
            workspacePersistence: RoutingWorkspacePersistenceStub()
        )
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = root
        appState.focusedAreaID[key] = browserArea.id
        appState.lastActiveTerminalPaneID[key] = terminalPane.id

        return RoutingHarness(
            appState: appState,
            controller: RichInputController(),
            key: key,
            area: browserArea,
            terminalPaneID: terminalPane.id,
            terminalTabID: terminalTab.id,
            browserSession: browserSession,
            terminalAreaIDOverride: terminalArea.id
        )
    }

    private init(
        appState: AppState,
        controller: RichInputController,
        key: WorktreeKey,
        area: TabArea,
        terminalPaneID: UUID,
        terminalTabID: UUID,
        browserSession: BrowserSession,
        terminalAreaIDOverride: UUID? = nil
    ) {
        self.appState = appState
        self.controller = controller
        self.key = key
        self.area = area
        self.terminalPaneID = terminalPaneID
        self.terminalTabID = terminalTabID
        self.browserSession = browserSession
        self.terminalAreaIDOverride = terminalAreaIDOverride
    }

    private let terminalAreaIDOverride: UUID?
}

private final class RoutingWorkspacePersistenceStub: WorkspacePersisting {
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { [] }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

@MainActor
private final class RoutingSelectionStoreStub: ActiveProjectSelectionStoring {
    func loadActiveProjectID() -> UUID? { nil }
    func saveActiveProjectID(_: UUID?) {}
    func loadActiveWorktreeIDs() -> [UUID: UUID] { [:] }
    func saveActiveWorktreeIDs(_: [UUID: UUID]) {}
}

@MainActor
private final class RoutingTerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
