import AppKit
import Foundation
import OSLog

private let cliAccessorLogger = Logger(subsystem: "app.muxy", category: "CLIAccessor")

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

    static func refreshInstalledCLIIfNeeded() async {
        guard !didRefreshInstalledCLI else { return }
        didRefreshInstalledCLI = true

        let wrapper = CLIWrapperScript.contents(installedAppPath: Bundle.main.bundleURL.path)
        let localPath = "/usr/local/bin"
        let home = NSHomeDirectory()
        let installationPaths = [localPath, "\(home)/bin", "\(home)/.local/bin"]
        var installedContents: [String: String] = [:]
        var failedPaths = migrateInstalledCLI(
            wrapper: wrapper,
            installationPaths: installationPaths,
            contentsAtPath: { path in
                let contents = installedWrapperContents(atPath: path)
                installedContents[path] = contents
                return contents
            },
            installWrapper: { writeWrapper($0, to: $1) }
        )

        guard failedPaths.contains(localPath) else {
            showMigrationFailures(failedPaths, home: home)
            return
        }
        let fallbackFailures = failedPaths.filter { $0 != localPath }
        guard CLIUpdatePromptPreferences.shouldPrompt(for: CLIWrapperScript.currentFormatVersion),
              await confirmUpdate()
        else {
            showMigrationFailures(fallbackFailures, home: home)
            return
        }
        guard let expectedInstalledContents = installedContents["\(localPath)/muxy"] else {
            showMigrationFailures(failedPaths, home: home)
            return
        }

        let success = await Task.detached(priority: .userInitiated) {
            runAdminInstall(
                wrapper: wrapper,
                expectedInstalledContents: expectedInstalledContents
            )
        }.value
        if success {
            failedPaths.removeAll { $0 == localPath }
        }
        guard failedPaths.isEmpty else {
            showMigrationFailures(failedPaths, home: home)
            return
        }
        ToastState.shared.show(
            title: L10n.string("CLI Updated"),
            body: L10n.string("The Muxy CLI now follows the installed Muxy app version automatically.")
        )
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

    static func migrationFailureLabels(_ paths: [String], home: String) -> [String] {
        paths.map { path in
            if path == "\(home)/bin" {
                return "~/bin/muxy"
            }
            if path == "\(home)/.local/bin" {
                return "~/.local/bin/muxy"
            }
            return "\(path)/muxy"
        }
    }

    private static func installedWrapperContents(atPath path: String) -> String? {
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        do {
            return try String(contentsOfFile: path, encoding: .utf8)
        } catch {
            cliAccessorLogger.error(
                "Failed to read installed CLI wrapper at \(path, privacy: .public): \(error.localizedDescription, privacy: .public)"
            )
            return nil
        }
    }

    static func writeWrapper(_ wrapper: String, to binPath: String) -> Bool {
        let dir = URL(fileURLWithPath: binPath)
        let target = dir.appendingPathComponent("muxy")
        let staged = dir.appendingPathComponent(".muxy.\(UUID().uuidString)")
        do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            try Data(wrapper.utf8).write(to: staged, options: .atomic)
            try FileManager.default.setAttributes(
                [.posixPermissions: FilePermissions.executable],
                ofItemAtPath: staged.path
            )
            if FileManager.default.fileExists(atPath: target.path) {
                _ = try FileManager.default.replaceItemAt(
                    target,
                    withItemAt: staged,
                    backupItemName: nil,
                    options: .usingNewMetadataOnly
                )
            } else {
                try FileManager.default.moveItem(at: staged, to: target)
            }
            return true
        } catch {
            try? FileManager.default.removeItem(at: staged)
            cliAccessorLogger.error(
                "Failed to install CLI wrapper at \(target.path, privacy: .public): \(error.localizedDescription, privacy: .public)"
            )
            return false
        }
    }

    nonisolated private static func runAdminInstall(
        wrapper: String,
        expectedInstalledContents: String? = nil
    ) -> Bool {
        let shellCommand = adminInstallShellCommand(
            wrapper: wrapper,
            expectedInstalledContents: expectedInstalledContents
        )
        let escapedForAppleScript = shellCommand
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        let source = "do shell script \"\(escapedForAppleScript)\" with administrator privileges"
        guard let script = NSAppleScript(source: source) else {
            cliAccessorLogger.error("Failed to create privileged CLI installation script")
            return false
        }
        var error: NSDictionary?
        script.executeAndReturnError(&error)
        guard let error else { return true }
        cliAccessorLogger.error("Privileged CLI installation failed: \(String(describing: error), privacy: .public)")
        return false
    }

    nonisolated static func adminInstallShellCommand(
        wrapper: String,
        expectedInstalledContents: String? = nil
    ) -> String {
        let quotedWrapper = ShellEscaper.quote(wrapper)
        var command = "set -e; PATH=/usr/bin:/bin:/usr/sbin:/sbin; export PATH; umask 022; "
            + "/bin/mkdir -p /usr/local/bin; "
            + "temp=$(/usr/bin/mktemp /usr/local/bin/.muxy.XXXXXX); "
            + "expected=''; "
            + "trap '/bin/rm -f \"$temp\"; [ -z \"$expected\" ] || /bin/rm -f \"$expected\"' EXIT HUP INT TERM; "
            + "/usr/bin/printf '%s' \(quotedWrapper) > \"$temp\"; "
            + "/bin/chmod 755 \"$temp\"; "
        if let expectedInstalledContents {
            let quotedExpectedContents = ShellEscaper.quote(expectedInstalledContents)
            command += "expected=$(/usr/bin/mktemp /usr/local/bin/.muxy.expected.XXXXXX); "
                + "/usr/bin/printf '%s' \(quotedExpectedContents) > \"$expected\"; "
                + "/usr/bin/cmp -s \"$expected\" /usr/local/bin/muxy; "
        }
        command += "/bin/mv -f \"$temp\" /usr/local/bin/muxy; "
            + "trap - EXIT HUP INT TERM"
        return command
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

    private static func showMigrationFailures(_ paths: [String], home: String) {
        guard !paths.isEmpty else { return }
        let labels = migrationFailureLabels(paths, home: home).joined(separator: ", ")
        ToastState.shared.show(
            title: L10n.string("CLI Update Incomplete"),
            body: L10n.string("Could not update \(labels). Use Muxy → Install CLI to try again.")
        )
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

    private static func confirmUpdate() async -> Bool {
        if let window = readyWindow() {
            return await confirmUpdate(on: window)
        }
        await waitForKeyWindow()
        guard let window = readyWindow() else { return false }
        return await confirmUpdate(on: window)
    }

    private static func confirmUpdate(on window: NSWindow) async -> Bool {
        let alert = NSAlert()
        alert.messageText = L10n.string("Update Muxy CLI?")
        alert.informativeText = L10n.string("""
        The installed Muxy CLI is an older standalone copy that cannot follow app updates.

        Update it to a wrapper that always runs the CLI bundled with your current \
        Muxy version. macOS will ask for your administrator password.
        """)
        alert.alertStyle = .informational
        alert.icon = NSApp.applicationIconImage
        alert.addButton(withTitle: L10n.string("Update"))
        alert.addButton(withTitle: L10n.string("Not Now"))
        alert.buttons[0].keyEquivalent = "\r"
        alert.buttons[1].keyEquivalent = "\u{1b}"
        alert.showsSuppressionButton = true
        alert.suppressionButton?.title = L10n.string("Don't ask again")

        return await withCheckedContinuation { continuation in
            alert.beginSheetModal(for: window) { response in
                if response != .alertFirstButtonReturn,
                   alert.suppressionButton?.state == .on
                {
                    CLIUpdatePromptPreferences.suppress(formatVersion: CLIWrapperScript.currentFormatVersion)
                }
                continuation.resume(returning: response == .alertFirstButtonReturn)
            }
        }
    }

    private static func readyWindow() -> NSWindow? {
        readyWindow(keyWindow: NSApp.keyWindow, mainWindow: NSApp.mainWindow)
    }

    static func readyWindow(keyWindow: NSWindow?, mainWindow: NSWindow?) -> NSWindow? {
        guard let window = mainWindow ?? keyWindow,
              window.sheetParent == nil,
              window.attachedSheet == nil
        else { return nil }
        return window
    }

    private static func waitForKeyWindow() async {
        let center = NotificationCenter.default
        let holder = ObserverHolder()
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            holder.token = center.addObserver(
                forName: NSWindow.didBecomeKeyNotification,
                object: nil,
                queue: .main
            ) { _ in
                MainActor.assumeIsolated {
                    guard readyWindow() != nil, let token = holder.token else { return }
                    center.removeObserver(token)
                    holder.token = nil
                    continuation.resume()
                }
            }
            guard readyWindow() != nil, let token = holder.token else { return }
            center.removeObserver(token)
            holder.token = nil
            continuation.resume()
        }
    }

    @MainActor
    private final class ObserverHolder {
        var token: NSObjectProtocol?
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
