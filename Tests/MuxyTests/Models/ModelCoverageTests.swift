import AppKit
import CoreGraphics
import Foundation
import Testing

@testable import Muxy
@testable import MuxyShared

@Suite("Model coverage", .serialized)
@MainActor
struct ModelCoverageTests {
    @Test("Project icon colors resolve ids, hex values, and foreground preference")
    func projectIconColorsResolveValues() {
        #expect(ProjectIconColor.palette.count == 12)
        #expect(ProjectIconColor.swatch(for: "blue")?.name == "Blue")
        #expect(ProjectIconColor.swatch(for: "#3e63dd")?.id == "blue")
        #expect(ProjectIconColor.swatch(for: nil) == nil)
        #expect(ProjectIconColor.swatch(for: "missing") == nil)

        let rgb = ProjectIconColor.rgb(fromHex: " #FFFFFF ")
        #expect(rgb?.0 == 1)
        #expect(rgb?.1 == 1)
        #expect(rgb?.2 == 1)
        #expect(ProjectIconColor.rgb(fromHex: "#XYZ") == nil)
        #expect(ProjectIconColor.rgb(fromHex: "#FFFF") == nil)
        #expect(ProjectIconColor.Swatch(id: "light", name: "Light", hex: "#FFFFFF").prefersDarkForeground)
        #expect(!ProjectIconColor.Swatch(id: "dark", name: "Dark", hex: "#111111").prefersDarkForeground)
        #expect(!ProjectIconColor.Swatch(id: "bad", name: "Bad", hex: "bad").prefersDarkForeground)
    }

    @Test("Random swatch avoids used identifiers and falls back to the full palette")
    func randomSwatchAvoidsUsedIdentifiers() {
        let allButBlue = Set(ProjectIconColor.palette.map(\.id)).subtracting(["blue"])
        #expect(ProjectIconColor.randomSwatch(excluding: allButBlue)?.id == "blue")

        let allByHex = Set(ProjectIconColor.palette.map(\.hex)).subtracting(["#3E63DD"])
        #expect(ProjectIconColor.randomSwatch(excluding: allByHex)?.id == "blue")

        let everyID = Set(ProjectIconColor.palette.map(\.id))
        #expect(ProjectIconColor.randomSwatch(excluding: everyID) != nil)

        let picked = ProjectIconColor.randomSwatch(excluding: ["unknown"])
        #expect(picked != nil)
    }

    @Test("UIMetrics exposes scaled values for every metric")
    func uiMetricsExposeScaledValues() {
        let scale = UIScale.shared
        let original = scale.preset
        defer { scale.preset = original }

        scale.preset = .regular

        #expect(UIMetrics.fontMicro == 8)
        #expect(UIMetrics.fontXS == 9)
        #expect(UIMetrics.fontCaption == 10)
        #expect(UIMetrics.fontFootnote == 11)
        #expect(UIMetrics.fontBody == 12)
        #expect(UIMetrics.fontEmphasis == 13)
        #expect(UIMetrics.fontHeadline == 14)
        #expect(UIMetrics.fontTitle == 15)
        #expect(UIMetrics.fontTitleLarge == 16)
        #expect(UIMetrics.fontDisplay == 20)
        #expect(UIMetrics.fontHero == 24)
        #expect(UIMetrics.fontMega == 28)
        #expect(UIMetrics.spacing1 == 2)
        #expect(UIMetrics.spacing2 == 4)
        #expect(UIMetrics.spacing3 == 6)
        #expect(UIMetrics.spacing4 == 8)
        #expect(UIMetrics.spacing5 == 10)
        #expect(UIMetrics.spacing6 == 12)
        #expect(UIMetrics.spacing7 == 16)
        #expect(UIMetrics.spacing8 == 20)
        #expect(UIMetrics.spacing9 == 24)
        #expect(UIMetrics.spacing10 == 32)
        #expect(UIMetrics.iconXS == 10)
        #expect(UIMetrics.iconSM == 12)
        #expect(UIMetrics.iconMD == 14)
        #expect(UIMetrics.iconLG == 16)
        #expect(UIMetrics.iconXL == 20)
        #expect(UIMetrics.iconXXL == 28)
        #expect(UIMetrics.controlSmall == 20)
        #expect(UIMetrics.controlMedium == 24)
        #expect(UIMetrics.controlLarge == 32)
        #expect(UIMetrics.resizeHandleHitArea == 18)
        #expect(UIMetrics.radiusSM == 4)
        #expect(UIMetrics.radiusMD == 6)
        #expect(UIMetrics.radiusLG == 8)
        #expect(UIMetrics.radiusXL == 10)
        #expect(UIMetrics.sidebarCollapsedWidth == 44)
        #expect(UIMetrics.sidebarExpandedWidth == 220)
        #expect(UIMetrics.tabBarHeight == 28)
        #expect(UIMetrics.headerHeight == 36)
        #expect(UIMetrics.titleBarHeight == 32)
    }

    @Test("Sidebar display modes expose storage defaults and routing")
    func sidebarDisplayModesExposeBehavior() {
        UserDefaults.standard.removeObject(forKey: SidebarCollapsedStyle.storageKey)
        UserDefaults.standard.removeObject(forKey: SidebarExpandedStyle.storageKey)
        defer {
            UserDefaults.standard.removeObject(forKey: SidebarCollapsedStyle.storageKey)
            UserDefaults.standard.removeObject(forKey: SidebarExpandedStyle.storageKey)
        }

        #expect(SidebarCollapsedStyle.allCases.map(\.id) == ["hidden", "icons"])
        #expect(SidebarCollapsedStyle.allCases.map(\.title) == ["Hidden", "Icons"])
        #expect(SidebarCollapsedStyle.current == .icons)
        UserDefaults.standard.set("hidden", forKey: SidebarCollapsedStyle.storageKey)
        #expect(SidebarCollapsedStyle.current == .hidden)

        #expect(SidebarExpandedStyle.allCases.map(\.id) == ["icons", "wide"])
        #expect(SidebarExpandedStyle.allCases.map(\.title) == ["Icons", "Wide"])
        #expect(SidebarExpandedStyle.current == .wide)
        UserDefaults.standard.set("icons", forKey: SidebarExpandedStyle.storageKey)
        #expect(SidebarExpandedStyle.current == .icons)
    }

    @Test("Worktree config decodes object, string, missing, and invalid setup formats")
    func worktreeConfigDecodesSupportedFormats() throws {
        let objectData = #"{"setup":[{"command":"swift build","name":"Build"}]}"#.data(using: .utf8)!
        let objectConfig = try JSONDecoder().decode(WorktreeConfig.self, from: objectData)
        #expect(objectConfig.setup.count == 1)
        #expect(objectConfig.setup[0].command == "swift build")
        #expect(objectConfig.setup[0].name == "Build")

        let stringData = #"{"setup":["swift test","swift build"]}"#.data(using: .utf8)!
        let stringConfig = try JSONDecoder().decode(WorktreeConfig.self, from: stringData)
        #expect(stringConfig.setup.map(\.command) == ["swift test", "swift build"])
        #expect(stringConfig.setup.allSatisfy { $0.name == nil })

        let invalidData = #"{"setup":true}"#.data(using: .utf8)!
        #expect(try JSONDecoder().decode(WorktreeConfig.self, from: invalidData).setup.isEmpty)

        let encoded = try JSONEncoder().encode(WorktreeConfig(setup: [
            WorktreeConfig.SetupCommand(command: "make", name: nil),
        ]))
        let decoded = try JSONDecoder().decode(WorktreeConfig.self, from: encoded)
        #expect(decoded.setup[0].command == "make")
    }

    @Test("Worktree config load reads project file and ignores missing or invalid files")
    func worktreeConfigLoadReadsProjectFile() throws {
        let projectURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-worktree-config-\(UUID().uuidString)")
        let muxyURL = projectURL.appendingPathComponent(".muxy")
        try FileManager.default.createDirectory(at: muxyURL, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: projectURL) }

        #expect(WorktreeConfig.load(fromProjectPath: projectURL.path) == nil)

        let configURL = muxyURL.appendingPathComponent("worktree.json")
        try #"{"setup":["bootstrap"]}"#.write(to: configURL, atomically: true, encoding: .utf8)
        #expect(WorktreeConfig.load(fromProjectPath: projectURL.path)?.setup.first?.command == "bootstrap")

        try "{".write(to: configURL, atomically: true, encoding: .utf8)
        #expect(WorktreeConfig.load(fromProjectPath: projectURL.path) == nil)
    }

    @Test("Layout config parses and discovers supported files")
    func layoutConfigParsesAndDiscoversSupportedFiles() throws {
        let projectURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-layout-config-\(UUID().uuidString)")
        let layoutURL = projectURL.appendingPathComponent(".muxy").appendingPathComponent("layouts")
        try FileManager.default.createDirectory(at: layoutURL, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: projectURL) }

        try """
        layout: vertical
        panes:
          - tabs:
              - name: Editor
                command:
                  - swift build
                  - swift test
              - npm run dev
          - layout: horizontal
            panes:
              - tabs:
                  - command: git status
        """.write(to: layoutURL.appendingPathComponent("B.yml"), atomically: true, encoding: .utf8)
        try #"{"tabs":["echo hi"]}"#.write(to: layoutURL.appendingPathComponent("a.json"), atomically: true, encoding: .utf8)
        try "ignored".write(to: layoutURL.appendingPathComponent("ignored.txt"), atomically: true, encoding: .utf8)

        let descriptors = LayoutConfig.discover(projectPath: projectURL.path)
        #expect(descriptors.map(\.name) == ["a", "B"])
        #expect(LayoutConfig.load(projectPath: projectURL.path, name: "missing") == nil)

        let config = try #require(LayoutConfig.load(projectPath: projectURL.path, name: "B"))
        #expect(config.root == .branch(layout: .vertical, panes: [
            .leaf(tabs: [
                .init(name: "Editor", command: "swift build && swift test"),
                .init(name: nil, command: "npm run dev"),
            ]),
            .branch(layout: .horizontal, panes: [
                .leaf(tabs: [.init(name: nil, command: "git status")]),
            ]),
        ]))

        #expect(LayoutConfig.parse(nil) == nil)
        #expect(LayoutConfig.parse(["tabs": []]) == nil)
        #expect(LayoutConfig.parse(["panes": []]) == nil)
        #expect(LayoutConfig.parse(["tabs": [["name": "  ", "command": []]]]) == .init(root: .leaf(tabs: [.init(name: nil, command: nil)])))
    }

    @Test("Terminal search display text and publishing follow query length rules")
    func terminalSearchPublishesNeedles() async throws {
        let state = TerminalSearchState()
        #expect(state.displayText == "")
        state.total = 12
        #expect(state.displayText == "12 matches")
        state.selected = 3
        #expect(state.displayText == "3 of 12")

        var values: [String] = []
        state.startPublishing { values.append($0) }

        state.needle = "abc"
        state.pushNeedle()
        try await Task.sleep(for: .milliseconds(50))
        #expect(values == ["abc"])

        state.needle = ""
        state.pushNeedle()
        try await Task.sleep(for: .milliseconds(50))
        #expect(values == ["abc", ""])

        state.needle = "ab"
        state.pushNeedle()
        try await Task.sleep(for: .milliseconds(350))
        #expect(values == ["abc", "", "ab"])

        state.stopPublishing()
        state.needle = "abcd"
        state.pushNeedle()
        try await Task.sleep(for: .milliseconds(50))
        #expect(values == ["abc", "", "ab"])
    }

    @Test("Terminal pane consumeRestoredLaunch returns startup launch and updates state")
    func terminalPaneConsumeRestoredLaunchReturnsStartupLaunch() {
        let pane = TerminalPaneState(
            id: UUID(),
            projectPath: "/repo",
            title: "Shell",
            initialWorkingDirectory: "/repo/worktree",
            startupCommand: "zsh",
            startupCommandInteractive: false,
            closesOnStartupCommandExit: false,
            externalEditorFilePath: "/repo/file.swift"
        )

        #expect(pane.title == "Shell")
        #expect(pane.currentWorkingDirectory == "/repo/worktree")
        #expect(pane.externalEditorFilePath == "/repo/file.swift")
        #expect(pane.searchState.displayText == "")

        let launch = pane.consumeRestoredLaunch()
        #expect(launch == TerminalPaneLaunch(command: "zsh", interactive: false, closesOnCommandExit: false))

        pane.setWorkingDirectory("/repo/other")
        #expect(pane.currentWorkingDirectory == "/repo/other")
    }

    @Test("Tab drag coordinator computes hover zones and drop actions")
    func tabDragCoordinatorComputesHoverAndActions() {
        let coordinator = TabDragCoordinator()
        let projectID = UUID()
        let tabID = UUID()
        let sourceAreaID = UUID()
        let targetAreaID = UUID()
        coordinator.setAreaFrames([targetAreaID: CGRect(x: 10, y: 20, width: 100, height: 80)], forProject: projectID)
        coordinator.beginDrag(tabID: tabID, sourceAreaID: sourceAreaID, projectID: projectID)

        coordinator.updatePosition(CGPoint(x: 20, y: 60))
        #expect(coordinator.hoveredAreaID == targetAreaID)
        #expect(coordinator.hoveredZone == .left)

        let result = coordinator.endDrag()
        #expect(result?.targetAreaID == targetAreaID)
        #expect(result?.zone == .left)
        if case let .moveTab(actionProjectID, request) = result?.action(projectID: projectID) {
            #expect(actionProjectID == projectID)
            if case let .toNewSplit(actionTabID, actionSourceAreaID, actionTargetAreaID, split) = request {
                #expect(actionTabID == tabID)
                #expect(actionSourceAreaID == sourceAreaID)
                #expect(actionTargetAreaID == targetAreaID)
                #expect(split.direction == .horizontal)
                #expect(split.position == .first)
            } else {
                Issue.record("Expected split move request")
            }
        } else {
            Issue.record("Expected move tab action")
        }

        let zones: [(CGPoint, DropZone)] = [
            (CGPoint(x: 100, y: 60), .right),
            (CGPoint(x: 60, y: 30), .top),
            (CGPoint(x: 60, y: 95), .bottom),
            (CGPoint(x: 60, y: 60), .center),
            (CGPoint(x: 6, y: 60), .left),
        ]

        for (point, zone) in zones {
            coordinator.beginDrag(tabID: tabID, sourceAreaID: sourceAreaID, projectID: projectID)
            coordinator.updatePosition(point)
            #expect(coordinator.hoveredZone == zone)
            _ = coordinator.endDrag()
        }

        coordinator.beginDrag(tabID: tabID, sourceAreaID: sourceAreaID, projectID: UUID())
        coordinator.updatePosition(CGPoint(x: 60, y: 60))
        #expect(coordinator.endDrag() == nil)
    }

    @Test("Muxy notification codable preserves source and read state")
    func muxyNotificationCodablePreservesValues() throws {
        let notification = MuxyNotification(
            paneID: UUID(),
            projectID: UUID(),
            worktreeID: UUID(),
            areaID: UUID(),
            tabID: UUID(),
            worktreePath: "/repo",
            source: .aiProvider("codex"),
            title: "Done",
            body: "Finished",
            isRead: true
        )

        let data = try JSONEncoder().encode(notification)
        let decoded = try JSONDecoder().decode(MuxyNotification.self, from: data)
        #expect(decoded.id == notification.id)
        #expect(decoded.source == .aiProvider("codex"))
        #expect(decoded.title == "Done")
        #expect(decoded.body == "Finished")
        #expect(decoded.isRead)

        for source in [MuxyNotification.Source.osc, .socket] {
            let data = try JSONEncoder().encode(source)
            #expect(try JSONDecoder().decode(MuxyNotification.Source.self, from: data) == source)
        }
    }

    @Test("Workspace DTO split trees encode and decode")
    func workspaceDTOSplitTreesRoundTrip() throws {
        let firstTab = TabDTO(id: UUID(), kind: .terminal, title: "Shell", isPinned: false, paneID: UUID())
        let secondTab = TabDTO(id: UUID(), kind: .vcs, title: "File", isPinned: true)
        let firstArea = TabAreaDTO(id: UUID(), projectPath: "/tmp/a", tabs: [firstTab], activeTabID: firstTab.id)
        let secondArea = TabAreaDTO(id: UUID(), projectPath: "/tmp/b", tabs: [secondTab], activeTabID: secondTab.id)
        let branch = SplitBranchDTO(
            id: UUID(),
            direction: .horizontal,
            ratio: 0.4,
            first: .tabArea(firstArea),
            second: .tabArea(secondArea)
        )
        let workspace = WorkspaceDTO(
            projectID: UUID(),
            worktreeID: UUID(),
            focusedAreaID: secondArea.id,
            root: .split(branch)
        )

        let decoded = try JSONDecoder().decode(WorkspaceDTO.self, from: JSONEncoder().encode(workspace))

        #expect(decoded.projectID == workspace.projectID)
        #expect(decoded.worktreeID == workspace.worktreeID)
        #expect(decoded.focusedAreaID == secondArea.id)
        if case let .split(decodedBranch) = decoded.root {
            #expect(decodedBranch.direction == .horizontal)
            #expect(decodedBranch.ratio == 0.4)
        } else {
            Issue.record("Expected split node")
        }
    }

    @Test("Toast show replaces message and delayed dismissal ignores cancelled tasks")
    func toastShowReplacesMessage() async throws {
        let state = ToastState.shared
        state.show("First")
        #expect(state.message == "First")
        state.show("Second")
        #expect(state.message == "Second")
        try await Task.sleep(for: .milliseconds(20))
        #expect(state.message == "Second")
    }

    @Test("Editor settings expose rich input defaults")
    func editorSettingsExposeDefaults() {
        #expect(EditorSettings.minLineHeightMultiplier < EditorSettings.maxLineHeightMultiplier)
        #expect(EditorSettings.defaultRichInputFontFamily == "SF Mono")

        let settings = EditorSettings.shared
        let originalFamily = settings.richInputFontFamily
        let originalMultiplier = settings.richInputLineHeightMultiplier
        let originalStrategy = settings.richInputImageStrategy
        defer {
            settings.richInputFontFamily = originalFamily
            settings.richInputLineHeightMultiplier = originalMultiplier
            settings.richInputImageStrategy = originalStrategy
        }

        settings.resetToDefaults()
        #expect(settings.richInputFontFamily == EditorSettings.defaultRichInputFontFamily)
        #expect(settings.richInputLineHeightMultiplier == EditorSettings.defaultRichInputLineHeightMultiplier)
        #expect(settings.richInputImageStrategy == .clipboard)

        #expect(EditorSettings.availableMonospacedFonts.allSatisfy { !$0.isEmpty })
    }

    @Test("Muxy theme exposes cached color facade values")
    func muxyThemeExposesFacadeValues() {
        _ = MuxyTheme.bg
        _ = MuxyTheme.nsBg
        _ = MuxyTheme.nsFg
        _ = MuxyTheme.nsFgMuted
        _ = MuxyTheme.fg
        _ = MuxyTheme.fgMuted
        _ = MuxyTheme.fgDim
        _ = MuxyTheme.surface
        _ = MuxyTheme.border
        _ = MuxyTheme.hover
        _ = MuxyTheme.accent
        _ = MuxyTheme.accentSoft
        _ = MuxyTheme.warning
        _ = MuxyTheme.diffAddFg
        _ = MuxyTheme.diffRemoveFg
        _ = MuxyTheme.diffHunkFg
        _ = MuxyTheme.diffAddBg
        _ = MuxyTheme.diffRemoveBg
        _ = MuxyTheme.diffHunkBg
        _ = MuxyTheme.nsDiffAdd
        _ = MuxyTheme.nsDiffRemove
        _ = MuxyTheme.nsDiffHunk
        _ = MuxyTheme.nsDiffString
        _ = MuxyTheme.nsDiffNumber
        _ = MuxyTheme.nsDiffComment
        #expect([.light, .dark].contains(MuxyTheme.colorScheme))
    }

    @Test("Rich input draft, strategy, and state preserve attachments")
    func richInputModelsPreserveAttachments() throws {
        #expect(RichInputDraft.empty.isEmpty)
        #expect(!RichInputDraft(
            text: "",
            fileAttachments: [URL(fileURLWithPath: "/tmp/a.txt")],
            imageAttachments: [],
            imagePlaceholderCounter: 0
        ).isEmpty)

        #expect(RichInputImageStrategy.allCases.map(\.id) == ["clipboard", "inlinePath"])
        #expect(RichInputImageStrategy.allCases.map(\.displayName) == ["Clipboard Paste", "Inline File Path"])
        #expect(RichInputImageStrategy.clipboard.description.contains("clipboard"))
        #expect(RichInputImageStrategy.inlinePath.description.contains("paths"))

        let state = RichInputState()
        let imageURL = URL(fileURLWithPath: "/tmp/image.png")
        #expect(state.nextImagePlaceholder(for: imageURL) == "[Image 1]")
        #expect(state.imageAttachments == [imageURL])

        let draft = RichInputDraft(
            text: "hello",
            fileAttachments: [URL(fileURLWithPath: "/tmp/file.txt")],
            imageAttachments: [imageURL],
            imagePlaceholderCounter: 4
        )
        state.apply(draft)
        #expect(state.text == "hello")
        #expect(state.fileAttachments == draft.fileAttachments)
        #expect(state.imageAttachments == draft.imageAttachments)
        #expect(state.imagePlaceholderCounter == 4)
        #expect(state.draft == draft)

        let data = try JSONEncoder().encode(draft)
        #expect(try JSONDecoder().decode(RichInputDraft.self, from: data) == draft)
    }
}
