import AppKit
import Foundation
import UserNotifications

@MainActor
enum CLIAccessor {
    private static var isRealAppBundle: Bool {
        ProcessInfo.processInfo.environment["XPC_SERVICE_NAME"] != nil
    }

    static func openProjectFromPath(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) {
        let url = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory), isDirectory.boolValue else { return }

        if let existing = projectStore.projects.first(where: { $0.path == url.path }) {
            if let primary = worktreeStore.primary(for: existing.id) {
                appState.selectProject(existing, worktree: primary)
                NSApp.activate(ignoringOtherApps: true)
                return
            }
        }

        let project = Project(
            name: url.lastPathComponent,
            path: url.path(percentEncoded: false),
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return }
        appState.selectProject(project, worktree: primary)
        NSApp.activate(ignoringOtherApps: true)
    }

    static func installCLI() {
        guard isRealAppBundle else {
            alert(title: "CLI Install Failed", body: "Please run from Muxy app, not swift run.")
            return
        }

        let bundleURL = Bundle.main.bundleURL
        let contentsURL = bundleURL.appendingPathComponent("Contents")
        let hasContents = FileManager.default.fileExists(atPath: contentsURL.path)
        let resourceURL: URL = hasContents
            ? bundleURL.appendingPathComponent("Contents/Resources/muxy")
            : bundleURL.appendingPathComponent("muxy")

        guard FileManager.default.fileExists(atPath: resourceURL.path) else {
            alert(title: "CLI Not Found", body: "The CLI script was not found at \(resourceURL.path)")
            return
        }

        let binPaths = ["/usr/local/bin", "\(NSHomeDirectory())/bin"]

        for binPath in binPaths {
            let targetURL = URL(fileURLWithPath: "\(binPath)/muxy")
            let binDir = URL(fileURLWithPath: binPath)

            do {
                if !FileManager.default.fileExists(atPath: binPath) {
                    try FileManager.default.createDirectory(
                        at: binDir,
                        withIntermediateDirectories: true
                    )
                }
                if FileManager.default.fileExists(atPath: targetURL.path) {
                    try FileManager.default.removeItem(at: targetURL)
                }
                try FileManager.default.copyItem(at: resourceURL, to: targetURL)
                try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: targetURL.path)

                let displayPath = binPath == "/usr/local/bin" ? "/usr/local/bin/muxy" : "~/bin/muxy"
                alert(
                    title: "CLI Installed",
                    body: "Run 'muxy .' or 'muxy /path/to/project'"
                )
                return
            } catch {
                continue
            }
        }

        alert(
            title: "CLI Installation Failed",
            body: "Could not install. Try: cp Muxy.app/Contents/Resources/muxy /usr/local/bin/"
        )
    }

    private static func alert(title: String, body: String) {
        guard isRealAppBundle else { return }
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = body
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    private static func showNotification(title: String, body: String) {
        guard isRealAppBundle else { return }

        func post() {
            let center = UNUserNotificationCenter.current()
            center.requestAuthorization(options: [.alert, .sound]) { _, _ in
                let content = UNMutableNotificationContent()
                content.title = title
                content.body = body
                content.sound = .default
                let request = UNNotificationRequest(
                    identifier: UUID().uuidString,
                    content: content,
                    trigger: nil
                )
                center.add(request)
            }
        }

        if Thread.isMainThread {
            post()
        } else {
            DispatchQueue.main.async {
                post()
            }
        }
    }
}
