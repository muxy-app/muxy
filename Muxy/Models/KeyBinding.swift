import AppKit
import SwiftUI

private struct ShortcutMetadata {
    let displayName: String
    let category: String
    let scope: ShortcutScope
}

enum ShortcutAction: String, Codable, CaseIterable, Identifiable {
    case newTab
    case closeTab
    case renameTab
    case pinUnpinTab
    case splitRight
    case splitDown
    case closePane
    case focusPaneLeft
    case focusPaneRight
    case focusPaneUp
    case focusPaneDown
    case cycleNextTabAcrossPanes
    case cyclePreviousTabAcrossPanes
    case nextTab
    case previousTab
    case toggleThemePicker
    case newProject
    case openProject
    case reloadConfig
    case selectTab1
    case selectTab2
    case selectTab3
    case selectTab4
    case selectTab5
    case selectTab6
    case selectTab7
    case selectTab8
    case selectTab9
    case nextProject
    case previousProject
    case selectProject1
    case selectProject2
    case selectProject3
    case selectProject4
    case selectProject5
    case selectProject6
    case selectProject7
    case selectProject8
    case selectProject9
    case findInTerminal
    case openVCSTab
    case quickOpen
    case findInFiles
    case switchWorktree
    case saveFile
    case toggleSidebar
    case toggleFileTree
    case toggleAIUsage
    case navigateBack
    case navigateForward

    static let allCases: [Self] = [
        .newTab,
        .closeTab,
        .renameTab,
        .pinUnpinTab,
        .splitRight,
        .splitDown,
        .closePane,
        .focusPaneLeft,
        .focusPaneRight,
        .focusPaneUp,
        .focusPaneDown,
        .cycleNextTabAcrossPanes,
        .cyclePreviousTabAcrossPanes,
        .nextTab,
        .previousTab,
        .toggleThemePicker,
        .openProject,
        .reloadConfig,
        .selectTab1,
        .selectTab2,
        .selectTab3,
        .selectTab4,
        .selectTab5,
        .selectTab6,
        .selectTab7,
        .selectTab8,
        .selectTab9,
        .nextProject,
        .previousProject,
        .selectProject1,
        .selectProject2,
        .selectProject3,
        .selectProject4,
        .selectProject5,
        .selectProject6,
        .selectProject7,
        .selectProject8,
        .selectProject9,
        .findInTerminal,
        .openVCSTab,
        .quickOpen,
        .findInFiles,
        .switchWorktree,
        .saveFile,
        .toggleSidebar,
        .toggleFileTree,
        .toggleAIUsage,
        .navigateBack,
        .navigateForward,
    ]

    var id: String { rawValue }

    private var metadata: ShortcutMetadata {
        switch self {
        case .newTab: ShortcutMetadata(displayName: "New Tab", category: "Tabs", scope: .mainWindow)
        case .closeTab: ShortcutMetadata(displayName: "Close Tab", category: "Tabs", scope: .mainWindow)
        case .renameTab: ShortcutMetadata(displayName: "Rename Tab", category: "Tabs", scope: .mainWindow)
        case .pinUnpinTab: ShortcutMetadata(displayName: "Pin/Unpin Tab", category: "Tabs", scope: .mainWindow)
        case .splitRight: ShortcutMetadata(displayName: "Split Right", category: "Panes", scope: .mainWindow)
        case .splitDown: ShortcutMetadata(displayName: "Split Down", category: "Panes", scope: .mainWindow)
        case .closePane: ShortcutMetadata(displayName: "Close Pane", category: "Panes", scope: .mainWindow)
        case .focusPaneLeft: ShortcutMetadata(displayName: "Focus Pane Left", category: "Panes", scope: .mainWindow)
        case .focusPaneRight: ShortcutMetadata(displayName: "Focus Pane Right", category: "Panes", scope: .mainWindow)
        case .focusPaneUp: ShortcutMetadata(displayName: "Focus Pane Up", category: "Panes", scope: .mainWindow)
        case .focusPaneDown: ShortcutMetadata(displayName: "Focus Pane Down", category: "Panes", scope: .mainWindow)
        case .cycleNextTabAcrossPanes: ShortcutMetadata(
                displayName: "Cycle Next Tab (All Panes)",
                category: "Tab Navigation",
                scope: .mainWindow
            )
        case .cyclePreviousTabAcrossPanes: ShortcutMetadata(
                displayName: "Cycle Previous Tab (All Panes)",
                category: "Tab Navigation",
                scope: .mainWindow
            )
        case .nextTab: ShortcutMetadata(displayName: "Next Tab", category: "Tab Navigation", scope: .mainWindow)
        case .previousTab: ShortcutMetadata(displayName: "Previous Tab", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab1: ShortcutMetadata(displayName: "Tab 1", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab2: ShortcutMetadata(displayName: "Tab 2", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab3: ShortcutMetadata(displayName: "Tab 3", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab4: ShortcutMetadata(displayName: "Tab 4", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab5: ShortcutMetadata(displayName: "Tab 5", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab6: ShortcutMetadata(displayName: "Tab 6", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab7: ShortcutMetadata(displayName: "Tab 7", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab8: ShortcutMetadata(displayName: "Tab 8", category: "Tab Navigation", scope: .mainWindow)
        case .selectTab9: ShortcutMetadata(displayName: "Tab 9", category: "Tab Navigation", scope: .mainWindow)
        case .nextProject: ShortcutMetadata(displayName: "Next Project", category: "Project Navigation", scope: .mainWindow)
        case .previousProject: ShortcutMetadata(displayName: "Previous Project", category: "Project Navigation", scope: .mainWindow)
        case .selectProject1: ShortcutMetadata(displayName: "Project 1", category: "Project Navigation", scope: .mainWindow)
        case .selectProject2: ShortcutMetadata(displayName: "Project 2", category: "Project Navigation", scope: .mainWindow)
        case .selectProject3: ShortcutMetadata(displayName: "Project 3", category: "Project Navigation", scope: .mainWindow)
        case .selectProject4: ShortcutMetadata(displayName: "Project 4", category: "Project Navigation", scope: .mainWindow)
        case .selectProject5: ShortcutMetadata(displayName: "Project 5", category: "Project Navigation", scope: .mainWindow)
        case .selectProject6: ShortcutMetadata(displayName: "Project 6", category: "Project Navigation", scope: .mainWindow)
        case .selectProject7: ShortcutMetadata(displayName: "Project 7", category: "Project Navigation", scope: .mainWindow)
        case .selectProject8: ShortcutMetadata(displayName: "Project 8", category: "Project Navigation", scope: .mainWindow)
        case .selectProject9: ShortcutMetadata(displayName: "Project 9", category: "Project Navigation", scope: .mainWindow)
        case .findInTerminal: ShortcutMetadata(displayName: "Find", category: "Terminal", scope: .mainWindow)
        case .openVCSTab: ShortcutMetadata(displayName: "Source Control", category: "App", scope: .mainWindow)
        case .quickOpen: ShortcutMetadata(displayName: "Quick Open", category: "App", scope: .mainWindow)
        case .findInFiles: ShortcutMetadata(displayName: "Find in Files", category: "App", scope: .mainWindow)
        case .switchWorktree: ShortcutMetadata(displayName: "Open Switcher", category: "Project Navigation", scope: .mainWindow)
        case .saveFile: ShortcutMetadata(displayName: "Save File", category: "Editor", scope: .mainWindow)
        case .toggleSidebar: ShortcutMetadata(displayName: "Toggle Sidebar", category: "App", scope: .mainWindow)
        case .toggleFileTree: ShortcutMetadata(displayName: "Toggle File Tree", category: "App", scope: .mainWindow)
        case .toggleAIUsage: ShortcutMetadata(displayName: "Toggle AI Usage", category: "App", scope: .mainWindow)
        case .navigateBack: ShortcutMetadata(displayName: "Navigate Back", category: "Navigation", scope: .mainWindow)
        case .navigateForward: ShortcutMetadata(displayName: "Navigate Forward", category: "Navigation", scope: .mainWindow)
        case .toggleThemePicker: ShortcutMetadata(displayName: "Theme Picker", category: "App", scope: .mainWindow)
        case .newProject: ShortcutMetadata(displayName: "New Project", category: "App", scope: .mainWindow)
        case .openProject: ShortcutMetadata(displayName: "Open Project", category: "App", scope: .mainWindow)
        case .reloadConfig: ShortcutMetadata(displayName: "Reload Configuration", category: "App", scope: .global)
        }
    }

    var displayName: String { metadata.displayName }
    var category: String { metadata.category }
    var scope: ShortcutScope { metadata.scope }

    static var categories: [String] {
        ["Tabs", "Panes", "Tab Navigation", "Project Navigation", "Navigation", "Terminal", "Editor", "App"]
    }

    static func tabAction(for index: Int) -> Self? {
        let actions: [Self] = [
            .selectTab1, .selectTab2, .selectTab3, .selectTab4, .selectTab5,
            .selectTab6, .selectTab7, .selectTab8, .selectTab9,
        ]
        guard index >= 1, index <= actions.count else { return nil }
        return actions[index - 1]
    }

    static func projectAction(for index: Int) -> Self? {
        let actions: [Self] = [
            .selectProject1, .selectProject2, .selectProject3, .selectProject4, .selectProject5,
            .selectProject6, .selectProject7, .selectProject8, .selectProject9,
        ]
        guard index >= 1, index <= actions.count else { return nil }
        return actions[index - 1]
    }

    var tabSelectionIndex: Int? {
        switch self {
        case .selectTab1: 0
        case .selectTab2: 1
        case .selectTab3: 2
        case .selectTab4: 3
        case .selectTab5: 4
        case .selectTab6: 5
        case .selectTab7: 6
        case .selectTab8: 7
        case .selectTab9: 8
        default: nil
        }
    }

    var projectSelectionIndex: Int? {
        switch self {
        case .selectProject1: 0
        case .selectProject2: 1
        case .selectProject3: 2
        case .selectProject4: 3
        case .selectProject5: 4
        case .selectProject6: 5
        case .selectProject7: 6
        case .selectProject8: 7
        case .selectProject9: 8
        default: nil
        }
    }
}

struct KeyBinding: Codable, Identifiable {
    let action: ShortcutAction
    var combo: KeyCombo

    var id: String { action.rawValue }

    static let defaults: [Self] = [
        Self(action: .newTab, combo: KeyCombo(key: "t", command: true)),
        Self(action: .closeTab, combo: KeyCombo(key: "w", command: true)),
        Self(action: .renameTab, combo: KeyCombo(key: "t", command: true, shift: true)),
        Self(action: .pinUnpinTab, combo: KeyCombo(key: "p", command: true, shift: true)),
        Self(action: .splitRight, combo: KeyCombo(key: "d", command: true)),
        Self(action: .splitDown, combo: KeyCombo(key: "d", command: true, shift: true)),
        Self(action: .closePane, combo: KeyCombo(key: "w", command: true, shift: true)),
        Self(action: .focusPaneLeft, combo: KeyCombo(key: KeyCombo.leftArrowKey, command: true, option: true)),
        Self(action: .focusPaneRight, combo: KeyCombo(key: KeyCombo.rightArrowKey, command: true, option: true)),
        Self(action: .focusPaneUp, combo: KeyCombo(key: KeyCombo.upArrowKey, command: true, option: true)),
        Self(action: .focusPaneDown, combo: KeyCombo(key: KeyCombo.downArrowKey, command: true, option: true)),
        Self(action: .cycleNextTabAcrossPanes, combo: KeyCombo(key: KeyCombo.tabKey, control: true)),
        Self(action: .cyclePreviousTabAcrossPanes, combo: KeyCombo(key: KeyCombo.tabKey, shift: true, control: true)),
        Self(action: .toggleThemePicker, combo: KeyCombo(key: "k", command: true, shift: true)),
        Self(action: .openVCSTab, combo: KeyCombo(key: "k", command: true)),
        Self(action: .openProject, combo: KeyCombo(key: "o", command: true)),
        Self(action: .reloadConfig, combo: KeyCombo(key: "r", command: true, shift: true)),
        Self(action: .nextTab, combo: KeyCombo(key: "]", command: true)),
        Self(action: .previousTab, combo: KeyCombo(key: "[", command: true)),
        Self(action: .selectTab1, combo: KeyCombo(key: "1", command: true)),
        Self(action: .selectTab2, combo: KeyCombo(key: "2", command: true)),
        Self(action: .selectTab3, combo: KeyCombo(key: "3", command: true)),
        Self(action: .selectTab4, combo: KeyCombo(key: "4", command: true)),
        Self(action: .selectTab5, combo: KeyCombo(key: "5", command: true)),
        Self(action: .selectTab6, combo: KeyCombo(key: "6", command: true)),
        Self(action: .selectTab7, combo: KeyCombo(key: "7", command: true)),
        Self(action: .selectTab8, combo: KeyCombo(key: "8", command: true)),
        Self(action: .selectTab9, combo: KeyCombo(key: "9", command: true)),
        Self(action: .nextProject, combo: KeyCombo(key: "]", control: true)),
        Self(action: .previousProject, combo: KeyCombo(key: "[", control: true)),
        Self(action: .selectProject1, combo: KeyCombo(key: "1", control: true)),
        Self(action: .selectProject2, combo: KeyCombo(key: "2", control: true)),
        Self(action: .selectProject3, combo: KeyCombo(key: "3", control: true)),
        Self(action: .selectProject4, combo: KeyCombo(key: "4", control: true)),
        Self(action: .selectProject5, combo: KeyCombo(key: "5", control: true)),
        Self(action: .selectProject6, combo: KeyCombo(key: "6", control: true)),
        Self(action: .selectProject7, combo: KeyCombo(key: "7", control: true)),
        Self(action: .selectProject8, combo: KeyCombo(key: "8", control: true)),
        Self(action: .selectProject9, combo: KeyCombo(key: "9", control: true)),
        Self(action: .findInTerminal, combo: KeyCombo(key: "f", command: true)),
        Self(action: .quickOpen, combo: KeyCombo(key: "p", command: true)),
        Self(action: .findInFiles, combo: KeyCombo(key: "f", command: true, shift: true)),
        Self(action: .switchWorktree, combo: KeyCombo(key: "o", command: true, shift: true)),
        Self(action: .saveFile, combo: KeyCombo(key: "s", command: true)),
        Self(action: .toggleSidebar, combo: KeyCombo(key: "b", command: true)),
        Self(action: .toggleFileTree, combo: KeyCombo(key: "e", command: true)),
        Self(action: .toggleAIUsage, combo: KeyCombo(key: "l", command: true)),
        Self(action: .navigateBack, combo: KeyCombo(key: KeyCombo.leftArrowKey, command: true, control: true)),
        Self(action: .navigateForward, combo: KeyCombo(key: KeyCombo.rightArrowKey, command: true, control: true)),
    ]
}
