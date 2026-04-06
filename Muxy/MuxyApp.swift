import AppKit
import SwiftUI

@main
struct MuxyApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    @State private var appState: AppState
    @State private var projectStore: ProjectStore
    private let updateService = UpdateService.shared

    init() {
        let environment = AppEnvironment.live
        _appState = State(
            initialValue: AppState(
                selectionStore: environment.selectionStore,
                terminalViews: environment.terminalViews,
                workspacePersistence: environment.workspacePersistence
            )
        )
        _projectStore = State(
            initialValue: ProjectStore(
                persistence: environment.projectPersistence
            )
        )
    }

    var body: some Scene {
        WindowGroup {
            MainWindow()
                .environment(appState)
                .environment(projectStore)
                .environment(GhosttyService.shared)
                .environment(MuxyConfig.shared)
                .environment(ThemeService.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
                .onAppear {
                    appDelegate.onTerminate = { [appState] in
                        appState.saveWorkspaces()
                    }
                    appState.onProjectsEmptied = { [projectStore] projectIDs in
                        for id in projectIDs {
                            projectStore.remove(id: id)
                        }
                    }
                }
        }
        .windowStyle(HiddenTitleBarWindowStyle())
        .defaultSize(width: 1200, height: 800)
        .commands {
            MuxyCommands(
                appState: appState,
                projectStore: projectStore,
                keyBindings: .shared,
                config: .shared,
                ghostty: .shared,
                updateService: .shared
            )
        }

        Window("Source Control", id: "vcs") {
            VCSWindowView()
                .environment(appState)
                .environment(projectStore)
                .environment(GhosttyService.shared)
                .preferredColorScheme(MuxyTheme.colorScheme)
        }
        .defaultSize(width: 700, height: 600)

        Settings {
            SettingsView()
                .preferredColorScheme(MuxyTheme.colorScheme)
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    var onTerminate: (() -> Void)?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
        setAppIcon()
        _ = GhosttyService.shared
        ThemeService.shared.applyDefaultThemeIfNeeded()
        UpdateService.shared.start()
        ModifierKeyMonitor.shared.start()
    }

    func applicationWillTerminate(_ notification: Notification) {
        onTerminate?()
    }

    @MainActor
    private func setAppIcon() {
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

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSView {
        let v = NSView()
        DispatchQueue.main.async {
            guard let w = v.window else { return }
            w.identifier = ShortcutContext.mainWindowIdentifier
            w.titlebarAppearsTransparent = true
            w.titleVisibility = .hidden
            w.styleMask.insert(.fullSizeContentView)
            w.isMovable = false
            w.isMovableByWindowBackground = false
            Self.applyWindowBackground(w)
            Self.repositionTrafficLights(in: w)
            context.coordinator.observe(window: w)
        }
        return v
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        guard let w = nsView.window else { return }
        Self.applyWindowBackground(w)
    }

    private static func applyWindowBackground(_ window: NSWindow) {
        let opacity = GhosttyService.shared.backgroundOpacity
        if opacity < 1.0 {
            window.isOpaque = false
            window.backgroundColor = .clear
        } else {
            window.isOpaque = true
            window.backgroundColor = MuxyTheme.nsBg
        }
    }

    static let trafficLightY: CGFloat = 3.5

    static func repositionTrafficLights(in window: NSWindow) {
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            guard let btn = window.standardWindowButton(button) else { continue }
            var frame = btn.frame
            frame.origin.y = trafficLightY
            btn.frame = frame
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
                NSWindow.didEnterFullScreenNotification,
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
