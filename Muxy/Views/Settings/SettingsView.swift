import SwiftUI

struct SettingsView: View {
    var body: some View {
        TabView {
            GeneralSettingsView()
                .tabItem { Label("General", systemImage: "gearshape") }
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
            AIAssistantSettingsView()
                .tabItem { Label("AI", systemImage: "sparkles") }
            AIUsageSettingsView()
                .tabItem { Label("AI Usage", systemImage: "chart.bar") }
        }
        .frame(width: 500, height: 500)
        .resetsSettingsFocusOnOutsideClick()
    }
}
