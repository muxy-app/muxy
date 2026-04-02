import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            KeyboardShortcutsSettingsView()
                .tabItem { Label("Shortcuts", systemImage: "keyboard") }
        }
        .frame(width: 500, height: 500)
    }
}
