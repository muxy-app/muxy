import AppKit
import Testing

@testable import Muxy

@Suite("Terminal context menu")
@MainActor
struct TerminalContextMenuTests {
    @Test("terminal settings item opens settings focused on Terminal")
    func terminalSettingsItemOpensTerminalSettings() throws {
        let notificationCenter = NotificationCenter()
        let coordinator = SettingsFocusCoordinator(notificationCenter: notificationCenter)
        let flag = TerminalSettingsOpenFlag()
        let observer = notificationCenter.addObserver(
            forName: .openSettingsModal,
            object: nil,
            queue: nil
        ) { _ in
            flag.didPost = true
        }
        defer { notificationCenter.removeObserver(observer) }
        let view = GhosttyTerminalNSView(workingDirectory: "/tmp")

        let menu = view.makeContextMenu(settingsFocusCoordinator: coordinator)
        let settingsIndex = try #require(menu.items.firstIndex {
            $0.title == L10n.string("Terminal Settings…")
        })
        let settingsItem = menu.items[settingsIndex]

        #expect(settingsIndex > 0)
        #expect(menu.items[settingsIndex - 1].isSeparatorItem)

        _ = settingsItem.target?.perform(settingsItem.action, with: settingsItem)

        #expect(flag.didPost)
        #expect(coordinator.consume(.terminal))
    }
}

private final class TerminalSettingsOpenFlag: @unchecked Sendable {
    var didPost = false
}
