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
        let bundleURL = Bundle.main.bundleURL
        var resourceURL: URL

        if bundleURL.pathExtension == "app" {
            resourceURL = bundleURL.appendingPathComponent("Contents/Resources/muxy-cli")
        } else {
            resourceURL = bundleURL.appendingPathComponent("Muxy_Muxy.bundle/muxy-cli")
            if !FileManager.default.fileExists(atPath: resourceURL.path) {
                resourceURL = bundleURL.appendingPathComponent("muxy-cli")
            }
        }

        guard FileManager.default.fileExists(atPath: resourceURL.path) else {
            alert(title: "CLI Not Found", body: "The CLI script was not found at \(resourceURL.path)")
            return
        }

        let home = NSHomeDirectory()

        let tryCopy = { (binPath: String, label: String) -> Bool in
            let target = URL(fileURLWithPath: "\(binPath)/muxy")
            let dir = URL(fileURLWithPath: binPath)
            if !FileManager.default.fileExists(atPath: binPath) {
                try? FileManager.default.createDirectory(
                    at: dir, withIntermediateDirectories: true
                )
            }
            if FileManager.default.fileExists(atPath: target.path) {
                try? FileManager.default.removeItem(at: target)
            }
            do {
                try FileManager.default.copyItem(at: resourceURL, to: target)
                try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: target.path)
                let pathNote = binPath == home + "/bin" || binPath == home + "/.local/bin"
                    ? "\n\nAdd to PATH:\n  export PATH=\"$PATH:\(binPath)\""
                    : ""
                alert(
                    title: "CLI Installed",
                    body: "Installed to: \(label)\nRun 'muxy .' or 'muxy /path/to/project'\(pathNote)"
                )
                return true
            } catch {
                return false
            }
        }

        // Try /usr/local/bin normally first
        if tryCopy("/usr/local/bin", "/usr/local/bin/muxy") { return }

        // If that fails, try with admin privileges via AppleScript
        let escaped = resourceURL.path.replacingOccurrences(of: "\"", with: "\\\"")
        let script = NSAppleScript(source: """
            do shell script "cp \\\"\(escaped)\\\" /usr/local/bin/muxy && chmod +x /usr/local/bin/muxy" with administrator privileges
        """)
        var error: NSDictionary?
        script?.executeAndReturnError(&error)
        if error == nil {
            alert(
                title: "CLI Installed",
                body: "Installed to: /usr/local/bin/muxy\nRun 'muxy .' or 'muxy /path/to/project'"
            )
            return
        }

        // Fallback to user-writable directories
        let fallbacks = [
            (path: "\(home)/bin", label: "~/bin/muxy"),
            (path: "\(home)/.local/bin", label: "~/.local/bin/muxy"),
        ]
        for (path, label) in fallbacks {
            guard !tryCopy(path, label) else { return }
        }

        alert(
            title: "CLI Installation Failed",
            body: """
            Could not install muxy to /usr/local/bin or any fallback directory.

            Try manually:
              sudo cp "\(resourceURL.path)" /usr/local/bin/muxy
              sudo chmod +x /usr/local/bin/muxy
            """
        )
    }

    private static func alert(title: String, body: String) {
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
