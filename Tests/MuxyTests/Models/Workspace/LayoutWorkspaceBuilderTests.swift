import Foundation
import Testing
import Yams

@testable import Muxy

@Suite("LayoutWorkspaceBuilder")
@MainActor
struct LayoutWorkspaceBuilderTests {
    private let testPath = "/tmp/test"

    @Test("returns nil for empty branch")
    func emptyBranch() {
        let config = LayoutConfig(root: .branch(layout: .horizontal, panes: []))
        #expect(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath) == nil)
    }

    @Test("single tab leaf")
    func singleTabLeaf() throws {
        let config = LayoutConfig(root: .leaf(tab: .init(name: "dev", command: "npm run dev")))
        let result = try #require(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath))
        guard case let .tabArea(area) = result.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(area.tabs.count == 1)
        let pane = try #require(area.tabs[0].content.pane)
        #expect(pane.title == "dev")
        #expect(pane.startupCommand == "npm run dev")
        #expect(pane.startupCommandInteractive == true)
        #expect(area.tabs[0].parentTabID == nil)
        #expect(result.focusedAreaID == area.id)
    }

    @Test("legacy extra tabs become independent top-level tabs")
    func legacyExtraTabs() throws {
        let config = LayoutConfig(
            root: .leaf(tab: .init(name: "one", command: nil)),
            legacyExtraTabs: [
                .init(name: nil, command: "echo hi"),
                .init(name: "three", command: nil),
            ]
        )
        let result = try #require(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath))
        guard case let .tabArea(area) = result.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(area.tabs.count == 3)
        #expect(area.activeTabID == area.tabs[0].id)
        #expect(area.tabs[0].content.pane?.title == "one")
        #expect(area.tabs[1].content.pane?.title == "echo")
        #expect(area.tabs[2].content.pane?.title == "three")
        #expect(area.tabs.allSatisfy { $0.parentTabID == nil })
    }

    @Test("two-pane horizontal split")
    func twoPaneHorizontal() throws {
        let config = LayoutConfig(root: .branch(layout: .horizontal, panes: [
            .leaf(tab: .init(name: "left", command: nil)),
            .leaf(tab: .init(name: "right", command: nil))
        ]))
        let result = try #require(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath))
        guard case let .split(branch) = result.root else {
            Issue.record("expected split")
            return
        }
        #expect(branch.direction == .horizontal)
        guard case let .tabArea(left) = branch.first,
              case let .tabArea(right) = branch.second
        else {
            Issue.record("expected two leaves")
            return
        }
        #expect(left.tabs[0].content.pane?.title == "left")
        #expect(right.tabs[0].content.pane?.title == "right")
        #expect(left.tabs[0].parentTabID == nil)
        #expect(right.tabs[0].parentTabID == left.tabs[0].id)
        #expect(result.focusedAreaID == left.id)
    }

    @Test("three panes produce nested splits")
    func threePanes() throws {
        let config = LayoutConfig(root: .branch(layout: .vertical, panes: [
            .leaf(tab: .init(name: "a", command: nil)),
            .leaf(tab: .init(name: "b", command: nil)),
            .leaf(tab: .init(name: "c", command: nil))
        ]))
        let result = try #require(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath))
        let areas = result.root.allAreas()
        #expect(areas.count == 3)
        #expect(areas.map { $0.tabs[0].content.pane?.title } == ["a", "b", "c"])
        let rootTabID = try #require(areas[0].tabs.first?.id)
        #expect(areas[0].tabs[0].parentTabID == nil)
        #expect(areas.dropFirst().allSatisfy { $0.tabs[0].parentTabID == rootTabID })
    }

    @Test("nested branch with mixed layouts")
    func nestedBranches() throws {
        let config = LayoutConfig(root: .branch(layout: .horizontal, panes: [
            .leaf(tab: .init(name: "left", command: nil)),
            .branch(layout: .vertical, panes: [
                .leaf(tab: .init(name: "top", command: nil)),
                .leaf(tab: .init(name: "bottom", command: nil))
            ])
        ]))
        let result = try #require(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath))
        guard case let .split(outer) = result.root else {
            Issue.record("expected outer split")
            return
        }
        #expect(outer.direction == .horizontal)
        guard case .tabArea = outer.first,
              case let .split(inner) = outer.second
        else {
            Issue.record("expected leaf + nested split")
            return
        }
        #expect(inner.direction == .vertical)
    }

    @Test("legacy extras from nested leaves preserve depth-first order as roots")
    func nestedLegacyExtraTabs() throws {
        let config = LayoutConfig(
            root: .branch(layout: .horizontal, panes: [
                .leaf(tab: .init(name: "left", command: nil)),
                .leaf(tab: .init(name: "right", command: nil)),
            ]),
            legacyExtraTabs: [
                .init(name: "left-extra", command: nil),
                .init(name: "right-extra", command: nil),
            ]
        )
        let result = try #require(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath))
        let areas = result.root.allAreas()
        #expect(areas[0].tabs.map { $0.content.pane?.title } == ["left", "left-extra", "right-extra"])
        #expect(areas[0].tabs.allSatisfy { $0.parentTabID == nil })
        #expect(areas[1].tabs[0].parentTabID == areas[0].tabs[0].id)
        #expect(areas[0].activeTabID == areas[0].tabs[0].id)
    }
}

@Suite("LayoutConfig")
struct LayoutConfigParsingTests {
    @Test("parses YAML with singular tab leaf")
    func parsesSingleTab() throws {
        let yaml = """
        tab:
          name: dev
          command: npm run dev
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .leaf(tab) = config.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(tab == .init(name: "dev", command: "npm run dev"))
        #expect(config.legacyExtraTabs.isEmpty)
    }

    @Test("parses nested panes")
    func parsesNested() throws {
        let yaml = """
        layout: horizontal
        panes:
          - tab:
              name: editor
              command: nvim
          - layout: vertical
            panes:
              - tab:
                  name: logs
                  command: tail -f log
              - tab: btop
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .branch(layout, panes) = config.root else {
            Issue.record("expected branch")
            return
        }
        #expect(layout == .horizontal)
        #expect(panes.count == 2)
        guard case let .branch(innerLayout, innerPanes) = panes[1] else {
            Issue.record("expected inner branch")
            return
        }
        #expect(innerLayout == .vertical)
        #expect(innerPanes.count == 2)
    }

    @Test("singular string tab is treated as command")
    func stringTab() throws {
        let yaml = """
        tab: htop
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .leaf(tab) = config.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(tab == .init(name: nil, command: "htop"))
    }

    @Test("array command joins with &&")
    func arrayCommand() throws {
        let yaml = """
        tab:
          name: setup
          command:
            - cd src
            - npm install
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .leaf(tab) = config.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(tab.command == "cd src && npm install")
    }

    @Test("legacy tabs keep first pane tab and collect extras depth-first")
    func legacyTabs() throws {
        let yaml = """
        layout: horizontal
        panes:
          - tabs:
              - name: editor
              - name: editor-extra
          - layout: vertical
            panes:
              - tabs:
                  - name: logs
                  - name: logs-extra
              - tab:
                  name: shell
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .branch(_, panes) = config.root,
              case let .leaf(first) = panes[0],
              case let .branch(_, nestedPanes) = panes[1],
              case let .leaf(second) = nestedPanes[0]
        else {
            Issue.record("expected nested layout")
            return
        }
        #expect(first.name == "editor")
        #expect(second.name == "logs")
        #expect(config.legacyExtraTabs.map(\.name) == ["editor-extra", "logs-extra"])
    }

    @Test("panes remain authoritative over tab fields")
    func panesAreAuthoritative() throws {
        let value = try Yams.load(yaml: """
        tab: ignored
        panes:
          - tab: used
        """)
        let config = try #require(LayoutConfig.parse(value))
        #expect(config.root == .branch(
            layout: .horizontal,
            panes: [.leaf(tab: .init(name: nil, command: "used"))]
        ))
    }
}
