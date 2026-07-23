import Foundation
import Testing

@testable import Muxy

@Suite("SocketCommandHandler")
@MainActor
struct SocketCommandHandlerTests {
    private let testPath = "/tmp/test"

    @Test("unknown command returns error")
    func unknownCommand() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("bogus", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("split-right returns new pane ID")
    func splitReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-down returns new pane ID")
    func splitDownReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-down", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-right with command returns new pane ID")
    func splitWithCommandReturnsNewPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right||echo hello", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("split-right preserves commands containing pipes")
    func splitPreservesCommandPipes() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("split-right||echo a | wc", appState: appState)
        let paneID = UUID(uuidString: result)
        #expect(paneID != nil)
        #expect(paneID.flatMap { pane(with: $0, appState: appState)?.startupCommand } == "(echo a | wc); exec \"$0\" -l")
    }

    @Test("split with startup command requires exec permission")
    func splitWithCommandRequiresExecPermission() {
        let plainSplit = SocketCommandHandler.requiredPermissions(
            command: "split-right",
            parts: ["split-right"]
        )
        let commandSplit = SocketCommandHandler.requiredPermissions(
            command: "split-right",
            parts: ["split-right", "", "echo hello"]
        )
        let whitespaceCommandSplit = SocketCommandHandler.requiredPermissions(
            command: "split-down",
            parts: ["split-down", "", "   "]
        )
        let targetedSplit = SocketCommandHandler.requiredPermissions(
            command: "split-right",
            parts: ["split-right", "", "", "--worktree", "feature"]
        )

        #expect(plainSplit == [.panesWrite])
        #expect(commandSplit == [.panesWrite, .commandsExec])
        #expect(whitespaceCommandSplit == [.panesWrite])
        #expect(targetedSplit == [.panesWrite])
    }

    @Test("split fails without active project")
    func splitFailsWithoutActiveProject() async {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let result = await SocketCommandHandler.handleRequest("split-right", appState: appState)
        #expect(result == "error:no active project")
    }

    @Test("send fails with missing args")
    func sendFailsMissingArgs() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("send-keys fails with unsupported key")
    func sendKeysFailsUnsupportedKey() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send-keys|\(UUID().uuidString)|F13", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("send-keys fails with missing key")
    func sendKeysFailsMissingKey() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("send-keys|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("close-pane fails with nonexistent pane")
    func closePaneFailsNonexistentPane() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("close-pane|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:pane not found"))
    }

    @Test("close-pane fails with invalid pane ID")
    func closePaneFailsInvalidPaneID() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("close-pane|not-a-uuid", appState: appState)
        #expect(result == "error:invalid pane ID")
    }

    @Test("rename-pane fails with nonexistent pane")
    func renamePaneFailsNonexistentPane() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("rename-pane|\(UUID().uuidString)|Test", appState: appState)
        #expect(result.hasPrefix("error:pane not found"))
    }

    @Test("rename-pane fails with missing title")
    func renamePaneFailsMissingTitle() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("rename-pane|\(UUID().uuidString)", appState: appState)
        #expect(result.hasPrefix("error:"))
    }

    @Test("list-panes returns empty when no panes")
    func listPanesEmpty() async {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let result = await SocketCommandHandler.handleRequest("list-panes", appState: appState)
        #expect(result.isEmpty)
    }

    @Test("list-panes returns tab-separated pane info")
    func listPanesReturnsPanes() async {
        let appState = makeAppState()
        let result = await SocketCommandHandler.handleRequest("list-panes", appState: appState)
        #expect(!result.isEmpty)
        let fields = result.components(separatedBy: "\t")
        #expect(fields.count >= 4)
        #expect(UUID(uuidString: fields[0]) != nil)
    }

    @Test("split-right with from pane targets correct area")
    func splitWithFromPane() async {
        let appState = makeAppState()
        let firstPaneID = appState.workspaceRoots.values.first!.allAreas().first!.tabs.first!.content.pane!.id
        let result = await SocketCommandHandler.handleRequest("split-right|" + firstPaneID.uuidString + "|", appState: appState)
        #expect(!result.hasPrefix("error:"))
        #expect(UUID(uuidString: result) != nil)
    }

    @Test("list-projects returns projects with active marker")
    func listProjects() async {
        let project = Project(name: "Test Project", path: testPath)
        let worktree = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState(projectID: project.id, worktreeID: worktree.id)
        let stores = makeStores(project: project, worktree: worktree)

        let result = await SocketCommandHandler.handleRequest(
            "list-projects",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result.contains(project.id.uuidString))
        #expect(result.contains("Test Project"))
        #expect(result.contains("\ttrue"))
    }

    @Test("switch-project selects matching project")
    func switchProject() async {
        let first = Project(name: "First", path: "/tmp/first")
        let second = Project(name: "Second", path: "/tmp/second")
        let firstWorktree = Worktree(name: first.name, path: first.path, isPrimary: true)
        let secondWorktree = Worktree(name: second.name, path: second.path, isPrimary: true)
        let appState = makeAppState(projectID: first.id, worktreeID: firstWorktree.id)
        let stores = makeStores(projects: [first, second], worktrees: [first.id: [firstWorktree], second.id: [secondWorktree]])

        let result = await SocketCommandHandler.handleRequest(
            "switch-project|Second",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result == "ok")
        #expect(appState.activeProjectID == second.id)
        #expect(appState.activeWorktreeID[second.id] == secondWorktree.id)
    }

    @Test("switch-worktree selects matching worktree")
    func switchWorktree() async {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/test-feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])

        let result = await SocketCommandHandler.handleRequest(
            "switch-worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result == "ok")
        #expect(appState.activeWorktreeID[project.id] == feature.id)
    }

    @Test("create-worktree creates and selects worktree")
    func createWorktree() async throws {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let createdPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-test-worktree-")
            .appendingPathComponent(UUID().uuidString)
            .path
        let stores = makeStores(
            projects: [project],
            worktrees: [project.id: [primary]],
            addGitWorktree: { _, _, _, _, _ in }
        )

        let result = await SocketCommandHandler.handleRequest(
            "create-worktree|Feature|feature|\(project.name)|\(createdPath)|true|main",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        let fields = result.components(separatedBy: "\t")
        #expect(fields.first == "ok")
        #expect(fields.count == 5)
        let worktree = try #require(stores.worktreeStore.list(for: project.id).first { $0.name == "Feature" })
        #expect(worktree.branch == "feature")
        #expect(worktree.path == createdPath)
        #expect(appState.activeWorktreeID[project.id] == worktree.id)
    }

    @Test("create-worktree resolves the configured template with the requested branch")
    func createWorktreeWithConfiguredTemplate() async throws {
        let projectPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-template-project-\(UUID().uuidString)")
            .path
        var project = Project(name: "Template Project", path: projectPath)
        project.preferredWorktreePathTemplate = "../{base-dir}.{branch}"
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(
            projects: [project],
            worktrees: [project.id: [primary]],
            addGitWorktree: { _, _, _, _, _ in }
        )

        let result = await SocketCommandHandler.handleRequest(
            "create-worktree|Display Name|feature/auth|\(project.name)||true|main",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        let worktree = try #require(stores.worktreeStore.list(for: project.id).first { $0.name == "Display Name" })
        let expectedPath = "\(projectPath).feature-auth"
        #expect(result.hasPrefix("ok\t"))
        #expect(worktree.path == expectedPath)
        #expect(worktree.branch == "feature/auth")
    }

    @Test("create-worktree rejects a configured template without branch")
    func createWorktreeRejectsStaticTemplate() async {
        var project = Project(name: "Template Project", path: testPath)
        project.preferredWorktreePathTemplate = "/tmp/worktrees"
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary]])

        let result = await SocketCommandHandler.handleRequest(
            "create-worktree|Feature|feature/auth|\(project.name)||true|main",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result == "error:Path template must include {branch}.")
        #expect(stores.worktreeStore.list(for: project.id).count == 1)
    }

    @Test("list-tabs includes active tab")
    func listTabs() async {
        let appState = makeAppState()
        appState.dispatch(.createTab(projectID: appState.activeProjectID!, areaID: nil))

        let result = await SocketCommandHandler.handleRequest("list-tabs", appState: appState)

        #expect(result.contains("\tterminal\t"))
        #expect(result.contains("\ttrue"))
    }

    @Test("switch-tab selects tab by index")
    func switchTabByIndex() async {
        let appState = makeAppState()
        let projectID = appState.activeProjectID!
        appState.dispatch(.createTab(projectID: projectID, areaID: nil))
        let area = appState.focusedArea(for: projectID)!
        let firstTabID = area.tabs[0].id

        let result = await SocketCommandHandler.handleRequest("switch-tab|0", appState: appState)

        #expect(result == "ok")
        #expect(area.activeTabID == firstTabID)
    }

    @Test("switch-tab reports invalid index")
    func switchTabByInvalidIndex() async {
        let appState = makeAppState()

        let result = await SocketCommandHandler.handleRequest("switch-tab|99", appState: appState)

        #expect(result.hasPrefix("error:tab not found"))
    }

    @Test("new-tab creates terminal tab")
    func newTab() async {
        let appState = makeAppState()
        let projectID = appState.activeProjectID!
        let before = appState.focusedArea(for: projectID)!.tabs.count

        let result = await SocketCommandHandler.handleRequest("new-tab", appState: appState)

        #expect(UUID(uuidString: result) != nil)
        #expect(appState.focusedArea(for: projectID)!.tabs.count == before + 1)
    }

    @Test("browser open preserves url without trailing framing fields")
    func browserOpenPreservesURL() async throws {
        let appState = makeAppState()

        let result = await SocketCommandHandler.handleRequest("browser.open|https://example.com", appState: appState)

        let tabID = try #require(UUID(uuidString: result))
        #expect(browserState(tabID: tabID, appState: appState)?.url?.absoluteString == "https://example.com")
    }

    @Test("browser open ignores legacy trailing empty framing field")
    func browserOpenIgnoresTrailingEmptyField() async throws {
        let appState = makeAppState()

        let result = await SocketCommandHandler.handleRequest("browser.open|https://example.com|", appState: appState)

        let tabID = try #require(UUID(uuidString: result))
        #expect(browserState(tabID: tabID, appState: appState)?.url?.absoluteString == "https://example.com")
    }

    @Test("browser open without url uses home page")
    func browserOpenWithoutURLUsesHomePage() async throws {
        let appState = makeAppState()
        let previous = BrowserPreferences.homePageURLString
        BrowserPreferences.homePageURLString = BrowserHomePage.blankURLString
        defer { BrowserPreferences.homePageURLString = previous }

        let result = await SocketCommandHandler.handleRequest("browser.open", appState: appState)

        let tabID = try #require(UUID(uuidString: result))
        #expect(browserState(tabID: tabID, appState: appState)?.url?.absoluteString == BrowserHomePage.blankURLString)
    }

    @Test("parseTargetFlags extracts project and worktree from remaining args")
    func parseTargetFlagsExtractsFlags() {
        let parsed = SocketCommandHandler.parseTargetFlags(["2", "--worktree", "feature", "--project", "App"])
        #expect(parsed.project == "App")
        #expect(parsed.worktree == "feature")
        #expect(parsed.remaining == ["2"])
    }

    @Test("parseTargetFlags only consumes trailing flags, preserving flag-like values")
    func parseTargetFlagsPreservesFlagLikeValues() {
        let parsed = SocketCommandHandler.parseTargetFlags(["https://x.com/--project/p", "--worktree", "feature"])
        #expect(parsed.project == nil)
        #expect(parsed.worktree == "feature")
        #expect(parsed.remaining == ["https://x.com/--project/p"])
    }

    @Test("new-tab --worktree creates tab in target without switching active worktree")
    func newTabTargetsWorktree() async {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])

        let result = await SocketCommandHandler.handleRequest(
            "new-tab|--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(UUID(uuidString: result) != nil)
        #expect(appState.activeWorktreeID[project.id] == primary.id)
        let featureKey = WorktreeKey(projectID: project.id, worktreeID: feature.id)
        #expect(appState.areas(for: featureKey).first?.tabs.contains { $0.id.uuidString == result } == true)
    }

    @Test("list-tabs --worktree lists only the target worktree's tabs")
    func listTabsTargetsWorktree() async {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])

        _ = await SocketCommandHandler.handleRequest(
            "new-tab|--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )
        let featureResult = await SocketCommandHandler.handleRequest(
            "list-tabs|--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )
        let activeResult = await SocketCommandHandler.handleRequest("list-tabs", appState: appState)

        let featureKey = WorktreeKey(projectID: project.id, worktreeID: feature.id)
        #expect(featureResult.split(separator: "\n").count == appState.areas(for: featureKey).flatMap(\.tabs).count)
        #expect(activeResult.split(separator: "\n").count == 1)
    }

    @Test("switch-tab --worktree selects within target without switching active worktree")
    func switchTabTargetsWorktree() async {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])

        _ = await SocketCommandHandler.handleRequest(
            "new-tab|--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )
        let result = await SocketCommandHandler.handleRequest(
            "switch-tab|0|--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result == "ok")
        #expect(appState.activeWorktreeID[project.id] == primary.id)
    }

    @Test("browser open --worktree creates browser tab in target without switching active worktree")
    func browserOpenTargetsWorktree() async throws {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])

        let result = await SocketCommandHandler.handleRequest(
            "browser.open|https://example.com|--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        let tabID = try #require(UUID(uuidString: result))
        #expect(appState.activeWorktreeID[project.id] == primary.id)
        let featureKey = WorktreeKey(projectID: project.id, worktreeID: feature.id)
        #expect(appState.areas(for: featureKey).first?.tabs.contains { $0.id == tabID } == true)
    }

    @Test("browser open --split --worktree opens the tab in the new split area")
    func browserOpenSplitTargetsNewArea() async throws {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])

        let result = await SocketCommandHandler.handleRequest(
            "browser.open|https://example.com|--split|--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        let tabID = try #require(UUID(uuidString: result))
        let featureKey = WorktreeKey(projectID: project.id, worktreeID: feature.id)
        let areas = appState.areas(for: featureKey)
        #expect(areas.count == 2)
        let hostingArea = try #require(areas.first { $0.tabs.contains { $0.id == tabID } })
        #expect(hostingArea.id == appState.focusedAreaID[featureKey])
        let browserTab = try #require(hostingArea.tabs.first { $0.id == tabID })
        let rootTab = try #require(appState.workspaceRoots[featureKey]?.allTabs().first { $0.parentTabID == nil })
        #expect(browserTab.content.browserState != nil)
        #expect(browserTab.parentTabID == rootTab.id)
        #expect(hostingArea.tabs.count == 1)
    }

    @Test("split-right --worktree splits the target worktree")
    func splitTargetsWorktreeInsteadOfCallingPane() async throws {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let feature = Worktree(name: "Feature", path: "/tmp/feature", branch: "feature", isPrimary: false)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary, feature]])
        let primaryKey = WorktreeKey(projectID: project.id, worktreeID: primary.id)
        let primaryPaneID = try #require(appState.areas(for: primaryKey).first?.tabs.first?.content.pane?.id)

        let result = await SocketCommandHandler.handleRequest(
            "split-right|\(primaryPaneID.uuidString)||--worktree|feature",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(UUID(uuidString: result) != nil)
        #expect(appState.activeWorktreeID[project.id] == primary.id)
        let featureKey = WorktreeKey(projectID: project.id, worktreeID: feature.id)
        #expect(appState.areas(for: primaryKey).count == 1)
        #expect(appState.areas(for: featureKey).count == 2)
    }

    @Test("new-tab --worktree reports unknown worktree")
    func newTabUnknownWorktree() async {
        let project = Project(name: "Test Project", path: testPath)
        let primary = Worktree(name: project.name, path: project.path, isPrimary: true)
        let appState = makeAppState(projectID: project.id, worktreeID: primary.id)
        let stores = makeStores(projects: [project], worktrees: [project.id: [primary]])

        let result = await SocketCommandHandler.handleRequest(
            "new-tab|--worktree|missing",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(result.hasPrefix("error:worktree not found"))
    }

    @Test("new-tab --project targets the project's preferred worktree")
    func newTabTargetsProject() async {
        let project = Project(name: "Other", path: "/tmp/other")
        let other = Worktree(name: project.name, path: project.path, isPrimary: true)
        let active = Project(name: "Active", path: testPath)
        let activeWorktree = Worktree(name: active.name, path: active.path, isPrimary: true)
        let appState = makeAppState(projectID: active.id, worktreeID: activeWorktree.id)
        let stores = makeStores(
            projects: [active, project],
            worktrees: [active.id: [activeWorktree], project.id: [other]]
        )

        let result = await SocketCommandHandler.handleRequest(
            "new-tab|--project|Other",
            appState: appState,
            projectStore: stores.projectStore,
            worktreeStore: stores.worktreeStore
        )

        #expect(UUID(uuidString: result) != nil)
        #expect(appState.activeProjectID == active.id)
        let otherKey = WorktreeKey(projectID: project.id, worktreeID: other.id)
        #expect(appState.areas(for: otherKey).first?.tabs.contains { $0.id.uuidString == result } == true)
    }

    @Test("direct tabs.open requires an identified extension")
    func directTabsOpenRequiresIdentify() async {
        let appState = makeAppState()
        let payload = Data(#"{"kind":"terminal"}"#.utf8).base64EncodedString()

        let result = await SocketCommandHandler.handleRequest("tabs.open|\(payload)", appState: appState)

        #expect(result == "error:identify required")
    }

    @Test("exec rejects an unidentified session")
    func execRejectsUnidentifiedSession() async {
        let appState = makeAppState()
        let payload = #"{"argv":["echo","hi"]}"#
        let encoded = Data(payload.utf8).base64EncodedString()
        let result = await SocketCommandHandler.handleRequest("exec|\(encoded)", appState: appState)
        #expect(result == "error:identify required")
    }

    @Test("exec denies an identified session without the commands:exec permission")
    func execDeniesWithoutPermission() async {
        let appState = makeAppState()
        let context = NotificationSocketServer.ClientContext(extensionID: "unloaded-ext")
        let payload = Data(#"{"argv":["echo","hi"]}"#.utf8).base64EncodedString()
        let result = await SocketCommandHandler.handleRequest(
            "exec|\(payload)",
            appState: appState,
            clientContext: context
        )
        #expect(result == "error:permission denied (commands:exec)")
    }

    @Test("exec base64 framing round-trips argv and stdin containing newlines and pipes")
    func execFramingRoundTrips() throws {
        let request = ExecRequest(
            argv: ["sh", "-c", "echo a | wc"],
            shell: nil,
            cwd: nil,
            env: nil,
            stdin: "line one\nline two | piped\n",
            timeoutMs: nil
        )
        let json: [String: Any] = [
            "argv": request.argv ?? [],
            "stdin": request.stdin ?? "",
        ]
        let encoded = try JSONSerialization.data(withJSONObject: json).base64EncodedString()
        #expect(!encoded.contains("\n"))
        #expect(!encoded.contains("|"))

        let decodedData = try #require(Data(base64Encoded: encoded))
        let decodedJSON = try #require(try JSONSerialization.jsonObject(with: decodedData) as? [String: Any])
        let decoded = try ExtensionBridgeShared.decodeExecRequest(decodedJSON)
        #expect(decoded.argv == ["sh", "-c", "echo a | wc"])
        #expect(decoded.stdin == "line one\nline two | piped\n")
    }

    @Test("exec result base64 framing round-trips stdout containing newlines")
    func execResultFramingRoundTrips() throws {
        let result = ExecResult(
            stdout: "first\nsecond | third\n",
            stderr: "",
            exitCode: 0,
            timedOut: false,
            truncated: false
        )
        let encoded = try JSONSerialization
            .data(withJSONObject: ExtensionBridgeShared.encodeExecResult(result))
            .base64EncodedString()
        let decodedData = try #require(Data(base64Encoded: encoded))
        let decoded = try #require(try JSONSerialization.jsonObject(with: decodedData) as? [String: Any])
        #expect(decoded["stdout"] as? String == "first\nsecond | third\n")
        #expect(decoded["exitCode"] as? Int == 0)
    }

    private func pane(with paneID: UUID, appState: AppState) -> TerminalPaneState? {
        for root in appState.workspaceRoots.values {
            for area in root.allAreas() {
                for tab in area.tabs where tab.content.pane?.id == paneID {
                    return tab.content.pane
                }
            }
        }
        return nil
    }

    private func browserState(tabID: UUID, appState: AppState) -> BrowserTabState? {
        for root in appState.workspaceRoots.values {
            for area in root.allAreas() {
                for tab in area.tabs where tab.id == tabID {
                    return tab.content.browserState
                }
            }
        }
        return nil
    }

    private func makeAppState(
        projectID: UUID = UUID(),
        worktreeID: UUID = UUID()
    ) -> AppState {
        let appState = AppState(
            selectionStore: SelectionStoreStub(),
            terminalViews: TerminalViewRemovingStub(),
            workspacePersistence: WorkspacePersistenceStub()
        )
        let key = WorktreeKey(projectID: projectID, worktreeID: worktreeID)
        let area = TabArea(projectPath: testPath)
        appState.activeProjectID = projectID
        appState.activeWorktreeID[projectID] = worktreeID
        appState.workspaceRoots[key] = .tabArea(area)
        appState.focusedAreaID[key] = area.id
        return appState
    }

    private func makeStores(
        project: Project,
        worktree: Worktree
    ) -> (projectStore: ProjectStore, worktreeStore: WorktreeStore) {
        makeStores(projects: [project], worktrees: [project.id: [worktree]])
    }

    private func makeStores(
        projects: [Project],
        worktrees: [UUID: [Worktree]],
        addGitWorktree: @escaping @Sendable (String, String, String, Bool, String?) async throws -> Void = { _, _, _, _, _ in }
    ) -> (projectStore: ProjectStore, worktreeStore: WorktreeStore) {
        let projectStore = ProjectStore(persistence: ProjectPersistenceSocketStub(projects: projects))
        let worktreeStore = WorktreeStore(
            persistence: WorktreePersistenceSocketStub(worktrees: worktrees),
            addGitWorktree: addGitWorktree,
            projects: projects
        )
        return (projectStore, worktreeStore)
    }
}

private final class WorkspacePersistenceStub: WorkspacePersisting {
    private var snapshots: [WorkspaceSnapshot] = []
    func loadWorkspaces() throws -> [WorkspaceSnapshot] { snapshots }
    func saveWorkspaces(_: [WorkspaceSnapshot]) throws {}
}

private final class ProjectPersistenceSocketStub: ProjectPersisting {
    private var projects: [Project]

    init(projects: [Project]) {
        self.projects = projects
    }

    func loadProjects() throws -> [Project] { projects }
    func saveProjects(_ projects: [Project]) throws { self.projects = projects }
}

private final class WorktreePersistenceSocketStub: WorktreePersisting {
    private var worktrees: [UUID: [Worktree]]

    init(worktrees: [UUID: [Worktree]]) {
        self.worktrees = worktrees
    }

    func loadWorktrees(projectID: UUID) throws -> [Worktree] { worktrees[projectID] ?? [] }
    func saveWorktrees(_ worktrees: [Worktree], projectID: UUID) throws { self.worktrees[projectID] = worktrees }
    func removeWorktrees(projectID: UUID) throws { worktrees[projectID] = nil }
}

@MainActor
private final class SelectionStoreStub: ActiveProjectSelectionStoring {
    private var activeProjectID: UUID?
    private var activeWorktreeIDs: [UUID: UUID] = [:]
    func loadActiveProjectID() -> UUID? { activeProjectID }
    func saveActiveProjectID(_ id: UUID?) { activeProjectID = id }
    func loadActiveWorktreeIDs() -> [UUID: UUID] { activeWorktreeIDs }
    func saveActiveWorktreeIDs(_ ids: [UUID: UUID]) { activeWorktreeIDs = ids }
}

@MainActor
private final class TerminalViewRemovingStub: TerminalViewRemoving {
    func removeView(for _: UUID) {}
    func needsConfirmQuit(for _: UUID) -> Bool { false }
}
