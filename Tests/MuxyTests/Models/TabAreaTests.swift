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
        area.createTab()
        #expect(area.tabs.count == 2)
        #expect(area.activeTabID != originalTabID)
        #expect(area.activeTabID == area.tabs[1].id)
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
    }

    @Test("createCommandTab ignores empty command")
    func createCommandTabEmptyCommand() {
        let area = TabArea(projectPath: testPath)
        let activeTabID = area.activeTabID
        area.createCommandTab(name: "Empty", command: " ")

        #expect(area.tabs.count == 1)
        #expect(area.activeTabID == activeTabID)
    }

    @Test("createVCSTab adds tab with VCS content")
    func createVCSTab() {
        let area = TabArea(projectPath: testPath)
        area.createVCSTab()
        #expect(area.tabs.count == 2)
        #expect(area.activeTab?.kind == .vcs)
    }

    @Test("createEditorTab adds tab with editor content")
    func createEditorTab() {
        let area = TabArea(projectPath: testPath)
        area.createEditorTab(filePath: "/tmp/test/file.swift")
        #expect(area.tabs.count == 2)
        #expect(area.activeTab?.kind == .editor)
    }

    @Test("createEditorTab reuses existing tab for same file path")
    func createEditorTabReuse() {
        let area = TabArea(projectPath: testPath)
        let filePath = "/tmp/test/file.swift"
        area.createEditorTab(filePath: filePath)
        let editorTabID = area.activeTabID

        area.createTab()
        #expect(area.activeTabID != editorTabID)

        area.createEditorTab(filePath: filePath)
        #expect(area.tabs.count == 3)
        #expect(area.activeTabID == editorTabID)
    }

    @Test("createExternalEditorTab adds terminal tab with launch command")
    func createExternalEditorTab() {
        let area = TabArea(projectPath: testPath)
        let filePath = "/tmp/test/file name.swift"
        area.createExternalEditorTab(filePath: filePath, command: "vim")

        let pane = area.activeTab?.content.pane
        #expect(area.activeTab?.kind == .terminal)
        #expect(pane?.externalEditorFilePath == filePath)
        #expect(pane?.startupCommand == "vim '/tmp/test/file name.swift'")
        #expect(pane?.startupCommandInteractive == true)
    }

    @Test("createExternalEditorTab supports file placeholder")
    func createExternalEditorTabPlaceholder() {
        let area = TabArea(projectPath: testPath)
        area.createExternalEditorTab(filePath: "/tmp/test/file.swift", command: "vim +10 {file}")

        #expect(area.activeTab?.content.pane?.startupCommand == "vim +10 /tmp/test/file.swift")
    }

    @Test("shellEscapedPath does not escape simple paths")
    func shellEscapedPathSimple() {
        let command = TabArea.editorLaunchCommand(command: "vim", filePath: "/tmp/test/file.swift")
        #expect(command == "vim /tmp/test/file.swift")
    }

    @Test("shellEscapedPath escapes paths with spaces")
    func shellEscapedPathSpaces() {
        let command = TabArea.editorLaunchCommand(command: "vim", filePath: "/tmp/test/my file.swift")
        #expect(command == "vim '/tmp/test/my file.swift'")
    }

    @Test("shellEscapedPath escapes paths with single quotes")
    func shellEscapedPathSingleQuotes() {
        let command = TabArea.editorLaunchCommand(command: "vim", filePath: "/tmp/test/it's a file.swift")
        #expect(command == "vim '/tmp/test/it'\\''s a file.swift'")
    }

    @Test("file placeholder uses raw path for user-controlled quoting")
    func filePlaceholderRawPath() {
        let command = TabArea.editorLaunchCommand(command: "vim \"{file}\"", filePath: "/tmp/test/my file.swift")
        #expect(command == "vim \"/tmp/test/my file.swift\"")
    }

    @Test("createExternalEditorTab reuses matching external editor tab")
    func createExternalEditorTabReuse() {
        let area = TabArea(projectPath: testPath)
        let filePath = "/tmp/test/file.swift"
        area.createExternalEditorTab(filePath: filePath, command: "vim -n")
        let editorTabID = area.activeTabID

        area.createTab()
        #expect(area.activeTabID != editorTabID)

        area.createExternalEditorTab(filePath: filePath, command: "vim")
        #expect(area.tabs.count == 3)
        #expect(area.activeTabID == editorTabID)
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
    func closeTabVCS() {
        let area = TabArea(projectPath: testPath)
        area.createVCSTab()
        let vcsTabID = area.activeTabID!
        let paneID = area.closeTab(vcsTabID)
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
