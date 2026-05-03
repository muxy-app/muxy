import Foundation
import Testing
import Yams

@testable import Muxy

@Suite("LayoutWorkspaceBuilder")
@MainActor
struct LayoutWorkspaceBuilderTests {
    private let testPath = "/tmp/test"

    @Test("returns nil for empty leaf")
    func emptyLeaf() {
        let config = LayoutConfig(root: .leaf(tabs: []))
        #expect(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath) == nil)
    }

    @Test("single tab leaf")
    func singleTabLeaf() throws {
        let config = LayoutConfig(root: .leaf(tabs: [
            .init(name: "dev", command: "npm run dev")
        ]))
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
        #expect(result.focusedAreaID == area.id)
    }

    @Test("multiple tabs preserve order with first focused")
    func multipleTabs() throws {
        let config = LayoutConfig(root: .leaf(tabs: [
            .init(name: "one", command: nil),
            .init(name: nil, command: "echo hi"),
            .init(name: "three", command: nil)
        ]))
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
    }

    @Test("two-pane horizontal split")
    func twoPaneHorizontal() throws {
        let config = LayoutConfig(root: .branch(layout: .horizontal, panes: [
            .leaf(tabs: [.init(name: "left", command: nil)]),
            .leaf(tabs: [.init(name: "right", command: nil)])
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
        #expect(result.focusedAreaID == left.id)
    }

    @Test("three panes produce nested splits")
    func threePanes() throws {
        let config = LayoutConfig(root: .branch(layout: .vertical, panes: [
            .leaf(tabs: [.init(name: "a", command: nil)]),
            .leaf(tabs: [.init(name: "b", command: nil)]),
            .leaf(tabs: [.init(name: "c", command: nil)])
        ]))
        let result = try #require(LayoutWorkspaceBuilder.build(config: config, projectPath: testPath))
        let areas = result.root.allAreas()
        #expect(areas.count == 3)
        #expect(areas.map { $0.tabs[0].content.pane?.title } == ["a", "b", "c"])
    }

    @Test("nested branch with mixed layouts")
    func nestedBranches() throws {
        let config = LayoutConfig(root: .branch(layout: .horizontal, panes: [
            .leaf(tabs: [.init(name: "left", command: nil)]),
            .branch(layout: .vertical, panes: [
                .leaf(tabs: [.init(name: "top", command: nil)]),
                .leaf(tabs: [.init(name: "bottom", command: nil)])
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
}

@Suite("LayoutConfig")
struct LayoutConfigParsingTests {
    @Test("parses YAML with single tab leaf")
    func parsesSingleTab() throws {
        let yaml = """
        tabs:
          - name: dev
            command: npm run dev
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .leaf(tabs) = config.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(tabs == [.init(name: "dev", command: "npm run dev")])
    }

    @Test("parses nested panes")
    func parsesNested() throws {
        let yaml = """
        layout: horizontal
        panes:
          - tabs:
              - name: editor
                command: nvim
          - layout: vertical
            panes:
              - tabs:
                  - name: logs
                    command: tail -f log
              - tabs:
                  - btop
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

    @Test("string tab is treated as command")
    func stringTab() throws {
        let yaml = """
        tabs:
          - htop
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .leaf(tabs) = config.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(tabs == [.init(name: nil, command: "htop")])
    }

    @Test("array command joins with &&")
    func arrayCommand() throws {
        let yaml = """
        tabs:
          - name: setup
            command:
              - cd src
              - npm install
        """
        let value = try Yams.load(yaml: yaml)
        let config = try #require(LayoutConfig.parse(value))
        guard case let .leaf(tabs) = config.root else {
            Issue.record("expected leaf")
            return
        }
        #expect(tabs[0].command == "cd src && npm install")
    }
}
