import AppKit
import SwiftUI

@main
struct MuxyApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var appState: AppState
    @State private var projectStore: ProjectStore

    init() {
        let environment = AppEnvironment.live
        _appState = State(
            initialValue: AppState(
                selectionStore: environment.selectionStore,
                terminalViews: environment.terminalViews
            ))
        _projectStore = State(
            initialValue: ProjectStore(
                persistence: environment.projectPersistence
            ))
    }

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appState)
                .environment(projectStore)
                .environment(GhosttyService.shared)
                .environment(MuxyConfig.shared)
                .environment(ThemeService.shared)
                .preferredColorScheme(.dark)
        }
        .windowStyle(HiddenTitleBarWindowStyle())
        .defaultSize(width: 1200, height: 800)
        .commands {
            MuxyCommands(appState: appState, config: .shared, ghostty: .shared)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
        setAppIcon()
        _ = GhosttyService.shared
    }

    @MainActor private func setAppIcon() {
        guard let url = Bundle.appResources.url(forResource: "AppIcon", withExtension: "png") else {
            return
        }
        guard let image = NSImage(contentsOf: url) else { return }
        image.size = NSSize(width: 512, height: 512)
        NSApp.applicationIconImage = image
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

struct WindowConfigurator: NSViewRepresentable {
    let configVersion: Int

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let v = NSView()
        DispatchQueue.main.async {
            guard let w = v.window else { return }
            w.titlebarAppearsTransparent = true
            w.titleVisibility = .hidden
            w.styleMask.insert(.fullSizeContentView)
            w.isMovableByWindowBackground = false
            w.backgroundColor = MuxyTheme.nsBg
            Self.repositionTrafficLights(in: w)
            context.coordinator.observe(window: w)
        }
        return v
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        nsView.window?.backgroundColor = MuxyTheme.nsBg
    }

    static func repositionTrafficLights(in window: NSWindow) {
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            guard let btn = window.standardWindowButton(button) else { continue }
            btn.superview?.frame.origin.y = -3
        }
    }

    final class Coordinator: NSObject {
        private var observations: [NSObjectProtocol] = []

        func observe(window: NSWindow) {
            guard observations.isEmpty else { return }
            let names: [Notification.Name] = [
                NSWindow.didResizeNotification,
                NSWindow.didEndLiveResizeNotification,
                NSWindow.didExitFullScreenNotification,
                NSWindow.didEnterFullScreenNotification
            ]
            for name in names {
                let token = NotificationCenter.default.addObserver(
                    forName: name,
                    object: window,
                    queue: .main
                ) { notification in
                    guard let w = notification.object as? NSWindow else { return }
                    MainActor.assumeIsolated {
                        WindowConfigurator.repositionTrafficLights(in: w)
                    }
                }
                observations.append(token)
            }
        }

        deinit {
            observations.forEach { NotificationCenter.default.removeObserver($0) }
        }
    }
}
