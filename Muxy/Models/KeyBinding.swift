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
    case nextPane
    case previousPane
    case toggleSidebar
    case toggleThemePicker
    case newProject
    case openProject
    case reloadConfig
    case selectTab1, selectTab2, selectTab3, selectTab4, selectTab5
    case selectTab6, selectTab7, selectTab8, selectTab9
    case selectProject1, selectProject2, selectProject3, selectProject4, selectProject5
    case selectProject6, selectProject7, selectProject8, selectProject9

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .newTab: return "New Tab"
        case .closeTab: return "Close Tab"
        case .renameTab: return "Rename Tab"
        case .pinUnpinTab: return "Pin/Unpin Tab"
        case .splitRight: return "Split Right"
        case .splitDown: return "Split Down"
        case .closePane: return "Close Pane"
        case .nextPane: return "Next Pane"
        case .previousPane: return "Previous Pane"
        case .toggleSidebar: return "Toggle Sidebar"
        case .toggleThemePicker: return "Theme Picker"
        case .newProject: return "New Project"
        case .openProject: return "Open Project"
        case .reloadConfig: return "Reload Configuration"
        case .selectTab1: return "Tab 1"
        case .selectTab2: return "Tab 2"
        case .selectTab3: return "Tab 3"
        case .selectTab4: return "Tab 4"
        case .selectTab5: return "Tab 5"
        case .selectTab6: return "Tab 6"
        case .selectTab7: return "Tab 7"
        case .selectTab8: return "Tab 8"
        case .selectTab9: return "Tab 9"
        case .selectProject1: return "Project 1"
        case .selectProject2: return "Project 2"
        case .selectProject3: return "Project 3"
        case .selectProject4: return "Project 4"
        case .selectProject5: return "Project 5"
        case .selectProject6: return "Project 6"
        case .selectProject7: return "Project 7"
        case .selectProject8: return "Project 8"
        case .selectProject9: return "Project 9"
        }
    }

    var category: String {
        switch self {
        case .newTab, .closeTab, .renameTab, .pinUnpinTab:
            return "Tabs"
        case .splitRight, .splitDown, .closePane, .nextPane, .previousPane:
            return "Panes"
        case .selectTab1, .selectTab2, .selectTab3, .selectTab4, .selectTab5,
            .selectTab6, .selectTab7, .selectTab8, .selectTab9:
            return "Tab Navigation"
        case .selectProject1, .selectProject2, .selectProject3, .selectProject4, .selectProject5,
            .selectProject6, .selectProject7, .selectProject8, .selectProject9:
            return "Project Navigation"
        case .toggleSidebar, .toggleThemePicker, .newProject, .openProject, .reloadConfig:
            return "App"
        }
    }

    static var categories: [String] {
        ["Tabs", "Panes", "Tab Navigation", "Project Navigation", "App"]
    }

    static func tabAction(for index: Int) -> ShortcutAction? {
        let actions: [ShortcutAction] = [
            .selectTab1, .selectTab2, .selectTab3, .selectTab4, .selectTab5,
            .selectTab6, .selectTab7, .selectTab8, .selectTab9,
        ]
        guard index >= 1, index <= actions.count else { return nil }
        return actions[index - 1]
    }

    static func projectAction(for index: Int) -> ShortcutAction? {
        let actions: [ShortcutAction] = [
            .selectProject1, .selectProject2, .selectProject3, .selectProject4, .selectProject5,
            .selectProject6, .selectProject7, .selectProject8, .selectProject9,
        ]
        guard index >= 1, index <= actions.count else { return nil }
        return actions[index - 1]
    }

    var scope: ShortcutScope {
        switch self {
        case .reloadConfig:
            return .global
        case .newTab,
            .closeTab,
            .renameTab,
            .pinUnpinTab,
            .splitRight,
            .splitDown,
            .closePane,
            .nextPane,
            .previousPane,
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
            .selectProject1,
            .selectProject2,
            .selectProject3,
            .selectProject4,
            .selectProject5,
            .selectProject6,
            .selectProject7,
            .selectProject8,
            .selectProject9:
            return .mainWindow
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
        case "[": return KeyEquivalent("[")
        case "]": return KeyEquivalent("]")
        case ",": return KeyEquivalent(",")
        default: return KeyEquivalent(Character(key))
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

    static let defaults: [KeyBinding] = [
        KeyBinding(action: .newTab, combo: KeyCombo(key: "t", command: true)),
        KeyBinding(action: .closeTab, combo: KeyCombo(key: "w", command: true)),
        KeyBinding(action: .renameTab, combo: KeyCombo(key: "t", command: true, shift: true)),
        KeyBinding(action: .pinUnpinTab, combo: KeyCombo(key: "p", command: true, shift: true)),
        KeyBinding(action: .splitRight, combo: KeyCombo(key: "d", command: true)),
        KeyBinding(action: .splitDown, combo: KeyCombo(key: "d", command: true, shift: true)),
        KeyBinding(action: .closePane, combo: KeyCombo(key: "w", command: true, shift: true)),
        KeyBinding(action: .nextPane, combo: KeyCombo(key: "]", command: true)),
        KeyBinding(action: .previousPane, combo: KeyCombo(key: "[", command: true)),
        KeyBinding(action: .toggleSidebar, combo: KeyCombo(key: "b", command: true)),
        KeyBinding(action: .toggleThemePicker, combo: KeyCombo(key: "k", command: true)),
        KeyBinding(action: .newProject, combo: KeyCombo(key: "n", command: true)),
        KeyBinding(action: .openProject, combo: KeyCombo(key: "o", command: true)),
        KeyBinding(action: .reloadConfig, combo: KeyCombo(key: "r", command: true, shift: true)),
        KeyBinding(action: .selectTab1, combo: KeyCombo(key: "1", command: true)),
        KeyBinding(action: .selectTab2, combo: KeyCombo(key: "2", command: true)),
        KeyBinding(action: .selectTab3, combo: KeyCombo(key: "3", command: true)),
        KeyBinding(action: .selectTab4, combo: KeyCombo(key: "4", command: true)),
        KeyBinding(action: .selectTab5, combo: KeyCombo(key: "5", command: true)),
        KeyBinding(action: .selectTab6, combo: KeyCombo(key: "6", command: true)),
        KeyBinding(action: .selectTab7, combo: KeyCombo(key: "7", command: true)),
        KeyBinding(action: .selectTab8, combo: KeyCombo(key: "8", command: true)),
        KeyBinding(action: .selectTab9, combo: KeyCombo(key: "9", command: true)),
        KeyBinding(action: .selectProject1, combo: KeyCombo(key: "1", control: true)),
        KeyBinding(action: .selectProject2, combo: KeyCombo(key: "2", control: true)),
        KeyBinding(action: .selectProject3, combo: KeyCombo(key: "3", control: true)),
        KeyBinding(action: .selectProject4, combo: KeyCombo(key: "4", control: true)),
        KeyBinding(action: .selectProject5, combo: KeyCombo(key: "5", control: true)),
        KeyBinding(action: .selectProject6, combo: KeyCombo(key: "6", control: true)),
        KeyBinding(action: .selectProject7, combo: KeyCombo(key: "7", control: true)),
        KeyBinding(action: .selectProject8, combo: KeyCombo(key: "8", control: true)),
        KeyBinding(action: .selectProject9, combo: KeyCombo(key: "9", control: true)),
    ]
}
