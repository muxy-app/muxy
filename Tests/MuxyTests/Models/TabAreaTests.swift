import Foundation
import Testing

@testable import Muxy

@Suite("TabArea")
@MainActor
struct TabAreaTests {
    private let testPath = "/tmp/test"

    @Test("init with projectPath creates one terminal tab")
    func initWithPath() {
        let area = TabArea(projectPath: testPath)
        #expect(area.tabs.count == 1)
        #expect(area.activeTabID != nil)
        #expect(area.activeTabID == area.tabs[0].id)
        #expect(area.tabs[0].kind == .terminal)
    }

    @Test("init with existingTab reuses the tab")
    func initWithExistingTab() {
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: testPath))
        let area = TabArea(projectPath: testPath, existingTab: tab)
        #expect(area.tabs.count == 1)
        #expect(area.tabs[0].id == tab.id)
        #expect(area.activeTabID == tab.id)
    }

    @Test("createTab appends and activates new tab")
    func createTab() {
        let area = TabArea(projectPath: testPath)
        let originalTabID = area.activeTabID
        let newID = area.createTab()
        #expect(area.tabs.count == 2)
        #expect(area.activeTabID != originalTabID)
        #expect(area.activeTabID == area.tabs[1].id)
        #expect(newID == area.tabs[1].id)
    }

    @Test("createExtensionTab returns the ExtensionTabState instance id")
    func createExtensionTabReturnsInstanceID() {
        let area = TabArea(projectPath: testPath)
        let instanceID = area.createExtensionTab(extensionID: "demo", tabTypeID: "panel", title: "Demo", data: nil)
        let created = area.tabs[1]
        #expect(created.content.extensionState?.id == instanceID)
        #expect(created.id != instanceID)
    }

    @Test("createCommandTab returns nil for an empty command")
    func createCommandTabReturnsNilForEmptyCommand() {
        let area = TabArea(projectPath: testPath)
        #expect(area.createCommandTab(name: "Empty", command: " ") == nil)
        #expect(area.createCommandTab(name: "Run", command: "ls") == area.tabs[1].id)
    }

    @Test("createCommandTab adds terminal tab with startup command")
    func createCommandTab() {
        let area = TabArea(projectPath: testPath)
        area.createCommandTab(name: "Server", command: " npm run dev ")

        let pane = area.activeTab?.content.pane
        #expect(area.tabs.count == 2)
        #expect(area.activeTab?.kind == .terminal)
        #expect(pane?.title == "Server")
        #expect(pane?.startupCommand == "npm run dev")
        #expect(pane?.closesOnStartupCommandExit == true)
    }

    @Test("createCommandTab can keep shell open after command")
    func createCommandTabKeepsShellOpen() {
        let area = TabArea(projectPath: testPath)
        area.createCommandTab(name: "Status", command: "git status", closesOnCommandExit: false)

        let pane = area.activeTab?.content.pane
        #expect(area.tabs.count == 2)
        #expect(pane?.startupCommand == "git status")
        #expect(pane?.closesOnStartupCommandExit == false)
    }

    @Test("createCommandTab ignores empty command")
    func createCommandTabEmptyCommand() {
        let area = TabArea(projectPath: testPath)
        let activeTabID = area.activeTabID
        area.createCommandTab(name: "Empty", command: " ")

        #expect(area.tabs.count == 1)
        #expect(area.activeTabID == activeTabID)
    }

    @Test("restoring terminal tab ignores stale working directory outside project")
    func restoringTerminalTabIgnoresOutsideWorkingDirectory() {
        let snapshot = TerminalTabSnapshot(
            kind: .terminal,
            customTitle: nil,
            colorID: nil,
            isPinned: false,
            projectPath: testPath,
            paneTitle: "~",
            currentWorkingDirectory: "/tmp"
        )

        let tab = TerminalTab(restoring: snapshot)

        #expect(tab.content.pane?.projectPath == testPath)
        #expect(tab.content.pane?.currentWorkingDirectory == nil)
    }

    @Test("restoring terminal tab keeps working directory inside project")
    func restoringTerminalTabKeepsInsideWorkingDirectory() {
        let snapshot = TerminalTabSnapshot(
            kind: .terminal,
            customTitle: nil,
            colorID: nil,
            isPinned: false,
            projectPath: testPath,
            paneTitle: "Sources",
            currentWorkingDirectory: "/tmp/test/Sources"
        )

        let tab = TerminalTab(restoring: snapshot)

        #expect(tab.content.pane?.currentWorkingDirectory == "/tmp/test/Sources")
    }

    @Test("TerminalTab restore preserves metadata and round-trips")
    func terminalTabRestorePreservesMetadata() {
        let paneID = UUID()
        let snapshot = TerminalTabSnapshot(
            kind: .terminal,
            id: UUID(),
            customTitle: "Shell",
            colorID: "green",
            isPinned: true,
            projectPath: testPath,
            paneTitle: "Stored",
            paneID: paneID,
            currentWorkingDirectory: "/outside"
        )

        let tab = TerminalTab(restoring: snapshot)
        let roundTrip = tab.snapshot()

        #expect(tab.id == snapshot.id)
        #expect(tab.customTitle == "Shell")
        #expect(tab.colorID == "green")
        #expect(tab.isPinned)
        #expect(tab.title == "Shell")
        #expect(tab.content.projectPath == testPath)
        #expect(tab.content.pane?.currentWorkingDirectory == nil)
        #expect(roundTrip.id == snapshot.id)
        #expect(roundTrip.customTitle == "Shell")
        #expect(roundTrip.colorID == "green")
        #expect(roundTrip.isPinned)
    }

    @Test("TerminalTab restore decodes legacy kinds as terminal")
    func terminalTabRestoreDecodesLegacyKindsAsTerminal() throws {
        for legacy in ["vcs", "diffViewer", "unknownKind"] {
            let json = """
            {
                "kind": "\(legacy)",
                "id": "\(UUID().uuidString)",
                "isPinned": false,
                "projectPath": "\(testPath)",
                "paneTitle": "Fallback"
            }
            """
            let snapshot = try JSONDecoder().decode(TerminalTabSnapshot.self, from: Data(json.utf8))
            let tab = TerminalTab(restoring: snapshot)

            #expect(tab.kind == .terminal)
            #expect(tab.content.pane?.title == "Fallback")
        }
    }

    @Test("TerminalTab content accessors return only matching state")
    func terminalTabContentAccessorsReturnOnlyMatchingState() {
        let terminal = TerminalTab(pane: TerminalPaneState(projectPath: testPath))

        #expect(terminal.content.pane != nil)
        #expect(terminal.content.projectPath == testPath)
    }

    @Test("closeTab removes tab and returns paneID for terminal")
    func closeTabTerminal() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        let firstTabID = area.tabs[0].id
        let paneID = area.closeTab(firstTabID)
        #expect(paneID != nil)
        #expect(area.tabs.count == 1)
    }

    @Test("closeTab on pinned tab returns nil")
    func closeTabPinned() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        let firstTabID = area.tabs[0].id
        area.togglePin(firstTabID)
        let paneID = area.closeTab(firstTabID)
        #expect(paneID == nil)
        #expect(area.tabs.count == 2)
    }

    @Test("closeTab non-terminal returns nil paneID")
    func closeTabNonTerminal() {
        let area = TabArea(projectPath: testPath)
        area.createExtensionTab(extensionID: "demo", tabTypeID: "panel", title: "Demo", data: nil)
        let extensionTabID = area.activeTabID!
        let paneID = area.closeTab(extensionTabID)
        #expect(paneID == nil)
        #expect(area.tabs.count == 1)
    }

    @Test("selectTab updates activeTabID")
    func selectTab() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        let firstTabID = area.tabs[0].id
        area.selectTab(firstTabID)
        #expect(area.activeTabID == firstTabID)
    }

    @Test("selectTabByIndex selects correct tab")
    func selectTabByIndex() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        area.createTab()
        area.selectTabByIndex(0)
        #expect(area.activeTabID == area.tabs[0].id)
    }

    @Test("selectTabByIndex out of bounds does nothing")
    func selectTabByIndexOutOfBounds() {
        let area = TabArea(projectPath: testPath)
        let originalID = area.activeTabID
        area.selectTabByIndex(99)
        #expect(area.activeTabID == originalID)
    }

    @Test("selectNextTab wraps around")
    func selectNextTab() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        area.createTab()
        area.selectTabByIndex(0)
        #expect(area.activeTabID == area.tabs[0].id)

        area.selectNextTab()
        #expect(area.activeTabID == area.tabs[1].id)

        area.selectNextTab()
        #expect(area.activeTabID == area.tabs[2].id)

        area.selectNextTab()
        #expect(area.activeTabID == area.tabs[0].id)
    }

    @Test("selectPreviousTab wraps around")
    func selectPreviousTab() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        area.createTab()
        area.selectTabByIndex(0)

        area.selectPreviousTab()
        #expect(area.activeTabID == area.tabs[2].id)
    }

    @Test("selectNextTab with single tab is no-op")
    func selectNextTabSingle() {
        let area = TabArea(projectPath: testPath)
        let originalID = area.activeTabID
        area.selectNextTab()
        #expect(area.activeTabID == originalID)
    }

    @Test("togglePin pins an unpinned tab and moves to front")
    func togglePinOn() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        let secondTabID = area.tabs[1].id
        area.togglePin(secondTabID)
        #expect(area.tabs[1].isPinned == false)
        #expect(area.tabs.first(where: { $0.id == secondTabID })?.isPinned == true)
        #expect(area.tabs[0].id == secondTabID)
    }

    @Test("togglePin unpins a pinned tab")
    func togglePinOff() {
        let area = TabArea(projectPath: testPath)
        let tabID = area.tabs[0].id
        area.togglePin(tabID)
        #expect(area.tabs[0].isPinned == true)
        area.togglePin(tabID)
        #expect(area.tabs.first(where: { $0.id == tabID })?.isPinned == false)
    }

    @Test("reorderTab changes tab order")
    func reorderTab() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        area.createTab()
        let thirdTabID = area.tabs[2].id
        area.reorderTab(fromOffsets: IndexSet(integer: 2), toOffset: 0)
        #expect(area.tabs[0].id == thirdTabID)
    }

    @Test("insertExistingTab adds and activates")
    func insertExistingTab() {
        let area = TabArea(projectPath: testPath)
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: testPath))
        area.insertExistingTab(tab)
        #expect(area.tabs.count == 2)
        #expect(area.activeTabID == tab.id)
    }

    @Test("insertExistingTab pinned tab inserts at front")
    func insertExistingTabPinned() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        let tab = TerminalTab(pane: TerminalPaneState(projectPath: testPath))
        tab.isPinned = true
        area.insertExistingTab(tab)
        #expect(area.tabs[0].id == tab.id)
    }

    @Test("closing active tab restores previous from history")
    func closeActiveRestoresPrevious() {
        let area = TabArea(projectPath: testPath)
        let firstTabID = area.tabs[0].id
        area.createTab()
        area.createTab()
        let thirdTabID = area.activeTabID!

        area.selectTab(firstTabID)
        area.selectTab(thirdTabID)

        _ = area.closeTab(thirdTabID)
        #expect(area.activeTabID == firstTabID)
    }

    @Test("createTabAdjacent left inserts before target")
    func createTabAdjacentLeft() {
        let area = TabArea(projectPath: testPath)
        area.createTab()
        let secondTabID = area.tabs[1].id
        area.createTabAdjacent(to: secondTabID, side: .left)
        #expect(area.tabs.count == 3)
        #expect(area.tabs[1].id != secondTabID)
        #expect(area.tabs[2].id == secondTabID)
    }

    @Test("createTabAdjacent right inserts after target")
    func createTabAdjacentRight() {
        let area = TabArea(projectPath: testPath)
        let firstTabID = area.tabs[0].id
        area.createTabAdjacent(to: firstTabID, side: .right)
        #expect(area.tabs.count == 2)
        #expect(area.tabs[0].id == firstTabID)
    }
}
