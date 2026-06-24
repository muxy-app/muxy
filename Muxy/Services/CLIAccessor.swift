import AppKit
import Foundation

@MainActor
enum CLIAccessor {
    static func openProjectFromPath(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) {
        let standardizedPath = URL(fileURLWithPath: path).standardizedFileURL.path
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(atPath: standardizedPath, isDirectory: &isDirectory),
              isDirectory.boolValue
        else { return }

        if let existing = projectStore.projects.first(where: { $0.path == standardizedPath }),
           let primary = worktreeStore.primary(for: existing.id)
        {
            projectGroupStore.addProjectToActiveGroup(projectID: existing.id)
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
        projectGroupStore.addProjectToActiveGroup(projectID: project.id)
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
        let appPath = Bundle.main.bundleURL.path
        let wrapper = CLIWrapperScript.contents(installedAppPath: appPath)

        guard confirmInstall() else { return }

        if writeWrapper(wrapper, to: "/usr/local/bin") {
            showInstalledAlert(label: "/usr/local/bin/muxy", pathNote: "")
            return
        }

        Task.detached(priority: .userInitiated) {
            let success = runAdminInstall(wrapper: wrapper)
            await MainActor.run {
                if success {
                    showInstalledAlert(label: "/usr/local/bin/muxy", pathNote: "")
                    return
                }
                if tryFallbackInstalls(wrapper: wrapper) { return }
                alert(
                    title: "CLI Installation Failed",
                    body: """
                    Could not install muxy to /usr/local/bin or any fallback directory.

                    Make sure /usr/local/bin exists and is writable, then try again.
                    """
                )
            }
        }
    }

    private static func writeWrapper(_ wrapper: String, to binPath: String) -> Bool {
        let target = URL(fileURLWithPath: "\(binPath)/muxy")
        let dir = URL(fileURLWithPath: binPath)
        if !FileManager.default.fileExists(atPath: binPath) {
            try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        do {
            try wrapper.write(to: target, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.executable],
                ofItemAtPath: target.path
            )
            return true
        } catch {
            return false
        }
    }

    nonisolated private static func runAdminInstall(wrapper: String) -> Bool {
        let quotedWrapper = ShellEscaper.escape(wrapper)
        let shellCommand = "mkdir -p /usr/local/bin && printf '%s' \(quotedWrapper) > /usr/local/bin/muxy && chmod +x /usr/local/bin/muxy"
        let escapedForAppleScript = shellCommand
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        let source = "do shell script \"\(escapedForAppleScript)\" with administrator privileges"
        guard let script = NSAppleScript(source: source) else { return false }
        var error: NSDictionary?
        script.executeAndReturnError(&error)
        return error == nil
    }

    private static func tryFallbackInstalls(wrapper: String) -> Bool {
        let home = NSHomeDirectory()
        let fallbacks = [
            (path: "\(home)/bin", label: "~/bin/muxy"),
            (path: "\(home)/.local/bin", label: "~/.local/bin/muxy"),
        ]
        for fallback in fallbacks {
            guard writeWrapper(wrapper, to: fallback.path) else {
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
