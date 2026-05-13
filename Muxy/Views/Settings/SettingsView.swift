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
            RecordingSettingsView()
                .tabItem { Label("Recording", systemImage: "mic") }
            NotificationSettingsView()
                .tabItem { Label("Notifications", systemImage: "bell") }
            MobileSettingsView()
                .tabItem { Label("Mobile", systemImage: "iphone") }
            AIAssistantSettingsView()
                .tabItem { Label("AI", systemImage: "sparkles") }
            AIUsageSettingsView()
                .tabItem { Label("AI Usage", systemImage: "chart.bar") }
        }
        .frame(minWidth: 720, minHeight: 560)
        .background(SettingsWindowConfigurator(minSize: NSSize(width: 720, height: 560)))
        .resetsSettingsFocusOnOutsideClick()
    }
}

private struct SettingsWindowConfigurator: NSViewRepresentable {
    let minSize: NSSize

    func makeNSView(context _: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async { [weak view] in
            guard let window = view?.window else { return }
            window.styleMask.insert(.resizable)
            window.minSize = minSize
            if window.frame.width < minSize.width || window.frame.height < minSize.height {
                var frame = window.frame
                frame.size.width = max(frame.size.width, minSize.width)
                frame.size.height = max(frame.size.height, minSize.height)
                window.setFrame(frame, display: true)
            }
        }
        return view
    }

    func updateNSView(_: NSView, context _: Context) {}
}
