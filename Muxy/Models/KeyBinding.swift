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

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .newTab: "New Tab"
        case .closeTab: "Close Tab"
        case .renameTab: "Rename Tab"
        case .pinUnpinTab: "Pin/Unpin Tab"
        case .splitRight: "Split Right"
        case .splitDown: "Split Down"
        case .closePane: "Close Pane"
        case .nextTab: "Next Tab"
        case .previousTab: "Previous Tab"
        case .toggleSidebar: "Toggle Sidebar"
        case .toggleThemePicker: "Theme Picker"
        case .newProject: "New Project"
        case .openProject: "Open Project"
        case .reloadConfig: "Reload Configuration"
        case .selectTab1: "Tab 1"
        case .selectTab2: "Tab 2"
        case .selectTab3: "Tab 3"
        case .selectTab4: "Tab 4"
        case .selectTab5: "Tab 5"
        case .selectTab6: "Tab 6"
        case .selectTab7: "Tab 7"
        case .selectTab8: "Tab 8"
        case .selectTab9: "Tab 9"
        case .nextProject: "Next Project"
        case .previousProject: "Previous Project"
        case .selectProject1: "Project 1"
        case .selectProject2: "Project 2"
        case .selectProject3: "Project 3"
        case .selectProject4: "Project 4"
        case .selectProject5: "Project 5"
        case .selectProject6: "Project 6"
        case .selectProject7: "Project 7"
        case .selectProject8: "Project 8"
        case .selectProject9: "Project 9"
        }
    }

    var category: String {
        switch self {
        case .newTab,
             .closeTab,
             .renameTab,
             .pinUnpinTab:
            "Tabs"
        case .splitRight,
             .splitDown,
             .closePane:
            "Panes"
        case .nextTab,
             .previousTab,
             .selectTab1,
             .selectTab2,
             .selectTab3,
             .selectTab4,
             .selectTab5,
             .selectTab6,
             .selectTab7,
             .selectTab8,
             .selectTab9:
            "Tab Navigation"
        case .nextProject,
             .previousProject,
             .selectProject1,
             .selectProject2,
             .selectProject3,
             .selectProject4,
             .selectProject5,
             .selectProject6,
             .selectProject7,
             .selectProject8,
             .selectProject9:
            "Project Navigation"
        case .toggleSidebar,
             .toggleThemePicker,
             .newProject,
             .openProject,
             .reloadConfig:
            "App"
        }
    }

    static var categories: [String] {
        ["Tabs", "Panes", "Tab Navigation", "Project Navigation", "App"]
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

    var scope: ShortcutScope {
        switch self {
        case .reloadConfig:
            .global
        case .newTab,
             .closeTab,
             .renameTab,
             .pinUnpinTab,
             .splitRight,
             .splitDown,
             .closePane,
             .nextTab,
             .previousTab,
             .toggleSidebar,
             .toggleThemePicker,
             .newProject,
             .openProject,
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
             .selectProject9:
            .mainWindow
        }
    }
}

enum ShortcutScope: String, Codable, CaseIterable {
    case global
    case mainWindow
}

struct KeyCombo: Codable, Equatable, Hashable {
    let key: String
    let modifiers: UInt

    init(key: String, modifiers: UInt) {
        self.key = key.lowercased()
        self.modifiers = modifiers
    }

    init(
        key: String, command: Bool = false, shift: Bool = false, control: Bool = false,
        option: Bool = false
    ) {
        self.key = key.lowercased()
        var flags: UInt = 0
        if command { flags |= NSEvent.ModifierFlags.command.rawValue }
        if shift { flags |= NSEvent.ModifierFlags.shift.rawValue }
        if control { flags |= NSEvent.ModifierFlags.control.rawValue }
        if option { flags |= NSEvent.ModifierFlags.option.rawValue }
        self.modifiers = flags
    }

    var nsModifierFlags: NSEvent.ModifierFlags {
        NSEvent.ModifierFlags(rawValue: modifiers).intersection(.deviceIndependentFlagsMask)
    }

    var swiftUIKeyEquivalent: KeyEquivalent {
        switch key {
        case "[": KeyEquivalent("[")
        case "]": KeyEquivalent("]")
        case ",": KeyEquivalent(",")
        default: KeyEquivalent(Character(key))
        }
    }

    var swiftUIModifiers: EventModifiers {
        var result: EventModifiers = []
        let flags = nsModifierFlags
        if flags.contains(.command) { result.insert(.command) }
        if flags.contains(.shift) { result.insert(.shift) }
        if flags.contains(.control) { result.insert(.control) }
        if flags.contains(.option) { result.insert(.option) }
        return result
    }

    var displayString: String {
        var parts = ""
        let flags = nsModifierFlags
        if flags.contains(.control) { parts += "⌃" }
        if flags.contains(.option) { parts += "⌥" }
        if flags.contains(.shift) { parts += "⇧" }
        if flags.contains(.command) { parts += "⌘" }
        parts += key.uppercased()
        return parts
    }

    func matches(event: NSEvent) -> Bool {
        let eventFlags = event.modifierFlags.intersection(.deviceIndependentFlagsMask).rawValue
        let eventKey = event.charactersIgnoringModifiers?.lowercased() ?? ""
        return eventKey == key && eventFlags == modifiers
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
    ]
}
