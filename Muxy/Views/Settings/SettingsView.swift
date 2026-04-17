import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            AppearanceSettingsView()
                .tabItem { Label("Appearance", systemImage: "paintbrush") }
            EditorSettingsView()
                .tabItem { Label("Editor", systemImage: "pencil.line") }
            KeyboardShortcutsSettingsView()
                .tabItem { Label("Shortcuts", systemImage: "keyboard") }
            NotificationSettingsView()
                .tabItem { Label("Notifications", systemImage: "bell") }
            MobileSettingsView()
                .tabItem { Label("Mobile", systemImage: "iphone") }
        }
        .frame(width: 500, height: 500)
    }
}
