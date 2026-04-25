import AppKit
import Foundation

@MainActor
enum CLIAccessor {
    static func openProjectFromPath(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore
    ) {
        let standardizedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: standardizedPath, isDirectory: &isDirectory),
              isDirectory.boolValue
        else { return }

        if let existing = projectStore.projects.first(where: { $0.path == standardizedPath }),
           let primary = worktreeStore.primary(for: existing.id)
        {
            appState.selectProject(existing, worktree: primary)
            activateApp()
            return
        }

        let url = URL(fileURLWithPath: standardizedPath)
        let project = Project(
            name: url.lastPathComponent,
            path: standardizedPath,
            sortOrder: projectStore.projects.count
        )
        projectStore.add(project)
        worktreeStore.ensurePrimary(for: project)
        guard let primary = worktreeStore.primary(for: project.id) else { return }
        appState.selectProject(project, worktree: primary)
        activateApp()
    }

    private static func activateApp() {
        let app = NSApplication.shared
        guard app.isRunning else { return }
        app.activate(ignoringOtherApps: true)
    }

    static func installCLI() {
        guard let resourceURL = Bundle.appResources.url(
            forResource: "muxy-cli",
            withExtension: ""
        )
        else {
            alert(title: "CLI Not Found", body: "The CLI script was not found in the app bundle.")
            return
        }

        guard confirmInstall() else { return }

        if copyScript(from: resourceURL, to: "/usr/local/bin", label: "/usr/local/bin/muxy") {
            showInstalledAlert(label: "/usr/local/bin/muxy", pathNote: "")
            return
        }

        Task.detached(priority: .userInitiated) {
            let success = runAdminInstall(resourceURL: resourceURL)
            await MainActor.run {
                if success {
                    showInstalledAlert(label: "/usr/local/bin/muxy", pathNote: "")
                    return
                }
                if tryFallbackInstalls(resourceURL: resourceURL) { return }
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
        }
    }

    private static func copyScript(from resourceURL: URL, to binPath: String, label: String) -> Bool {
        let target = URL(fileURLWithPath: "\(binPath)/muxy")
        let dir = URL(fileURLWithPath: binPath)
        if !FileManager.default.fileExists(atPath: binPath) {
            try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        if FileManager.default.fileExists(atPath: target.path) {
            try? FileManager.default.removeItem(at: target)
        }
        do {
            try FileManager.default.copyItem(at: resourceURL, to: target)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o755],
                ofItemAtPath: target.path
            )
            return true
        } catch {
            return false
        }
    }

    nonisolated private static func runAdminInstall(resourceURL: URL) -> Bool {
        let quotedSource = ShellEscaper.escape(resourceURL.path)
        let shellCommand = "mkdir -p /usr/local/bin && cp \(quotedSource) /usr/local/bin/muxy && chmod +x /usr/local/bin/muxy"
        let escapedForAppleScript = shellCommand
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        let source = "do shell script \"\(escapedForAppleScript)\" with administrator privileges"
        guard let script = NSAppleScript(source: source) else { return false }
        var error: NSDictionary?
        script.executeAndReturnError(&error)
        return error == nil
    }

    private static func tryFallbackInstalls(resourceURL: URL) -> Bool {
        let home = NSHomeDirectory()
        let fallbacks = [
            (path: "\(home)/bin", label: "~/bin/muxy"),
            (path: "\(home)/.local/bin", label: "~/.local/bin/muxy"),
        ]
        for fallback in fallbacks {
            guard copyScript(from: resourceURL, to: fallback.path, label: fallback.label) else {
                continue
            }
            let pathNote = "\n\nAdd to PATH:\n  export PATH=\"$PATH:\(fallback.path)\""
            showInstalledAlert(label: fallback.label, pathNote: pathNote)
            return true
        }
        return false
    }

    private static func showInstalledAlert(label: String, pathNote: String) {
        alert(
            title: "CLI Installed",
            body: "Installed to: \(label)\nRun 'muxy .' or 'muxy /path/to/project'\(pathNote)"
        )
    }

    private static func confirmInstall() -> Bool {
        let alert = NSAlert()
        alert.messageText = "Install Muxy CLI?"
        alert.informativeText = """
        This will install the 'muxy' command-line tool to /usr/local/bin so you \
        can launch projects from your terminal (e.g. 'muxy .').

        If /usr/local/bin is not writable, you will be prompted for your \
        administrator password. If that is declined, Muxy will fall back to \
        ~/bin or ~/.local/bin.
        """
        alert.alertStyle = .informational
        alert.addButton(withTitle: "Install")
        alert.addButton(withTitle: "Cancel")
        return alert.runModal() == .alertFirstButtonReturn
    }

    private static func alert(title: String, body: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = body
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}
