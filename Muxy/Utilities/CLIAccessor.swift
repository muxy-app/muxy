import AppKit
import Foundation

@MainActor
enum CLIAccessor {
    private static var didRefreshInstalledCLI = false

    static func openProjectFromPath(
        _ path: String,
        appState: AppState,
        projectStore: ProjectStore,
        worktreeStore: WorktreeStore,
        projectGroupStore: ProjectGroupStore
    ) {
        guard ProjectOpenService.confirmProjectPath(
            path,
            appState: appState,
            projectStore: projectStore,
            worktreeStore: worktreeStore,
            projectGroupStore: projectGroupStore
        )
        else {
            return
        }
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
                if tryFallbackInstalls(wrapper: wrapper) {
                    return
                }
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

    static func refreshInstalledCLIIfNeeded() {
        guard !didRefreshInstalledCLI else { return }
        didRefreshInstalledCLI = true

        let wrapper = CLIWrapperScript.contents(installedAppPath: Bundle.main.bundleURL.path)
        let localPath = "/usr/local/bin"
        let home = NSHomeDirectory()
        let installationPaths = [localPath, "\(home)/bin", "\(home)/.local/bin"]
        let failedPaths = migrateInstalledCLI(
            wrapper: wrapper,
            installationPaths: installationPaths,
            contentsAtPath: { try? String(contentsOfFile: $0, encoding: .utf8) },
            installWrapper: { writeWrapper($0, to: $1) }
        )

        guard failedPaths.contains(localPath), confirmUpdate() else { return }

        Task.detached(priority: .userInitiated) {
            let success = runAdminInstall(wrapper: wrapper)
            await MainActor.run {
                if success {
                    alert(
                        title: "CLI Updated",
                        body: "The Muxy CLI now follows the installed Muxy app version automatically."
                    )
                    return
                }
                alert(
                    title: "CLI Update Failed",
                    body: "Could not update /usr/local/bin/muxy. Use Muxy → Install CLI to try again."
                )
            }
        }
    }

    static func migrateInstalledCLI(
        wrapper: String,
        installationPaths: [String],
        contentsAtPath: (String) -> String?,
        installWrapper: (String, String) -> Bool
    ) -> [String] {
        var failedPaths: [String] = []

        for binPath in installationPaths {
            let targetPath = "\(binPath)/muxy"
            guard let installed = contentsAtPath(targetPath),
                  CLIWrapperScript.requiresMigration(installed)
            else {
                continue
            }
            guard installWrapper(wrapper, binPath) else {
                failedPaths.append(binPath)
                continue
            }
        }

        return failedPaths
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
        let shellCommand = "set -e; mkdir -p /usr/local/bin; "
            + "temp=$(mktemp /usr/local/bin/.muxy.XXXXXX); "
            + "trap 'rm -f \"$temp\"' EXIT; "
            + "printf '%s' \(quotedWrapper) > \"$temp\"; "
            + "chmod 755 \"$temp\"; mv -f \"$temp\" /usr/local/bin/muxy; trap - EXIT"
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
        alert.messageText = L10n.string("Install Muxy CLI?")
        alert.informativeText = L10n.string("""
        This will install the 'muxy' command-line tool to /usr/local/bin so you \
        can launch projects from your terminal (e.g. 'muxy .').

        If /usr/local/bin is not writable, you will be prompted for your \
        administrator password. If that is declined, Muxy will fall back to \
        ~/bin or ~/.local/bin.
        """)
        alert.alertStyle = .informational
        alert.addButton(withTitle: L10n.string("Install"))
        alert.addButton(withTitle: L10n.string("Cancel"))
        return alert.runModal() == .alertFirstButtonReturn
    }

    private static func confirmUpdate() -> Bool {
        let alert = NSAlert()
        alert.messageText = L10n.string("Update Muxy CLI?")
        alert.informativeText = L10n.string("""
        The installed Muxy CLI is an older standalone copy that cannot follow app updates.

        Update it to a wrapper that always runs the CLI bundled with your current \
        Muxy version. macOS will ask for your administrator password.
        """)
        alert.alertStyle = .informational
        alert.addButton(withTitle: L10n.string("Update"))
        alert.addButton(withTitle: L10n.string("Not Now"))
        return alert.runModal() == .alertFirstButtonReturn
    }

    private static func alert(
        title: LocalizedStringResource,
        body: LocalizedStringResource
    ) {
        let alert = NSAlert()
        alert.messageText = L10n.string(title)
        alert.informativeText = L10n.string(body)
        alert.alertStyle = .informational
        alert.addButton(withTitle: L10n.string("OK"))
        alert.runModal()
    }
}
