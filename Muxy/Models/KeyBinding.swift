import AppKit
import SwiftUI

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
    case nextTab
    case previousTab
    case toggleSidebar
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

    var id: String { rawValue }

    private var metadata: (displayName: String, category: String, scope: ShortcutScope) {
        switch self {
        case .newTab: ("New Tab", "Tabs", .mainWindow)
        case .closeTab: ("Close Tab", "Tabs", .mainWindow)
        case .renameTab: ("Rename Tab", "Tabs", .mainWindow)
        case .pinUnpinTab: ("Pin/Unpin Tab", "Tabs", .mainWindow)
        case .splitRight: ("Split Right", "Panes", .mainWindow)
        case .splitDown: ("Split Down", "Panes", .mainWindow)
        case .closePane: ("Close Pane", "Panes", .mainWindow)
        case .focusPaneLeft: ("Focus Pane Left", "Panes", .mainWindow)
        case .focusPaneRight: ("Focus Pane Right", "Panes", .mainWindow)
        case .focusPaneUp: ("Focus Pane Up", "Panes", .mainWindow)
        case .focusPaneDown: ("Focus Pane Down", "Panes", .mainWindow)
        case .nextTab: ("Next Tab", "Tab Navigation", .mainWindow)
        case .previousTab: ("Previous Tab", "Tab Navigation", .mainWindow)
        case .selectTab1: ("Tab 1", "Tab Navigation", .mainWindow)
        case .selectTab2: ("Tab 2", "Tab Navigation", .mainWindow)
        case .selectTab3: ("Tab 3", "Tab Navigation", .mainWindow)
        case .selectTab4: ("Tab 4", "Tab Navigation", .mainWindow)
        case .selectTab5: ("Tab 5", "Tab Navigation", .mainWindow)
        case .selectTab6: ("Tab 6", "Tab Navigation", .mainWindow)
        case .selectTab7: ("Tab 7", "Tab Navigation", .mainWindow)
        case .selectTab8: ("Tab 8", "Tab Navigation", .mainWindow)
        case .selectTab9: ("Tab 9", "Tab Navigation", .mainWindow)
        case .nextProject: ("Next Project", "Project Navigation", .mainWindow)
        case .previousProject: ("Previous Project", "Project Navigation", .mainWindow)
        case .selectProject1: ("Project 1", "Project Navigation", .mainWindow)
        case .selectProject2: ("Project 2", "Project Navigation", .mainWindow)
        case .selectProject3: ("Project 3", "Project Navigation", .mainWindow)
        case .selectProject4: ("Project 4", "Project Navigation", .mainWindow)
        case .selectProject5: ("Project 5", "Project Navigation", .mainWindow)
        case .selectProject6: ("Project 6", "Project Navigation", .mainWindow)
        case .selectProject7: ("Project 7", "Project Navigation", .mainWindow)
        case .selectProject8: ("Project 8", "Project Navigation", .mainWindow)
        case .selectProject9: ("Project 9", "Project Navigation", .mainWindow)
        case .findInTerminal: ("Find", "Terminal", .mainWindow)
        case .toggleSidebar: ("Toggle Sidebar", "App", .mainWindow)
        case .toggleThemePicker: ("Theme Picker", "App", .mainWindow)
        case .newProject: ("New Project", "App", .mainWindow)
        case .openProject: ("Open Project", "App", .mainWindow)
        case .reloadConfig: ("Reload Configuration", "App", .global)
        }
    }

    var displayName: String { metadata.displayName }
    var category: String { metadata.category }
    var scope: ShortcutScope { metadata.scope }

    static var categories: [String] {
        ["Tabs", "Panes", "Tab Navigation", "Project Navigation", "Terminal", "App"]
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
        Self(action: .toggleSidebar, combo: KeyCombo(key: "b", command: true)),
        Self(action: .toggleThemePicker, combo: KeyCombo(key: "k", command: true)),
        Self(action: .newProject, combo: KeyCombo(key: "n", command: true)),
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
    ]
}
