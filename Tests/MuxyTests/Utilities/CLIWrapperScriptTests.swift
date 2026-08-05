import AppKit
import Foundation
import Testing

@testable import Muxy

@Suite("CLIWrapperScript")
struct CLIWrapperScriptTests {
    @Test("wrapper execs the bundled script rather than embedding the CLI body")
    func wrapperExecsBundledScript() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(wrapper.hasPrefix("#!/bin/bash"))
        #expect(wrapper.contains("# Muxy CLI wrapper version \(CLIWrapperScript.currentFormatVersion)"))
        #expect(wrapper.contains("exec \"$SCRIPT\" \"$@\""))
        #expect(wrapper.contains(CLIWrapperScript.bundledScriptRelativePath))
        #expect(!wrapper.contains("send_command"))
    }

    @Test("wrapper resolves the app by bundle id so it survives moves")
    func wrapperResolvesByBundleID() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(wrapper.contains("kMDItemCFBundleIdentifier == \(CLIWrapperScript.bundleIdentifier)"))
        #expect(wrapper.contains("mdfind"))
    }

    @Test("wrapper honors MUXY_APP_PATH and falls back to standard locations")
    func wrapperHonorsOverrideAndFallbacks() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(wrapper.contains("${MUXY_APP_PATH:-}"))
        #expect(wrapper.contains("/Applications/Muxy.app"))
        #expect(wrapper.contains("$HOME/Applications/Muxy.app"))
    }

    @Test("captured app path with spaces is shell-quoted")
    func capturedAppPathIsQuoted() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Users/a/My Apps/Muxy.app")
        #expect(wrapper.contains("'/Users/a/My Apps/Muxy.app'"))
    }

    @Test("identifies only legacy Muxy CLI installations for migration")
    func identifiesLegacyInstallations() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        let legacy = """
        #!/bin/bash
        # Muxy CLI wrapper - Usage: muxy /path/to/project
        open "muxy://open?path=$1"
        """
        let copiedCLI = """
        #!/bin/bash
        # Muxy CLI wrapper
        SOCKET="${MUXY_SOCKET_PATH:-muxy.sock}"
        """

        #expect(!CLIWrapperScript.requiresMigration(wrapper))
        #expect(CLIWrapperScript.requiresMigration(legacy))
        #expect(CLIWrapperScript.requiresMigration(copiedCLI))
    }

    @Test("does not claim unrelated executables")
    func rejectsUnmanagedInstallations() {
        let unrelated = """
        #!/bin/bash
        open "muxy://open?path=$1"
        """

        #expect(!CLIWrapperScript.requiresMigration(unrelated))
    }

    @Test("runtime wrapper format versions support future migrations")
    func runtimeWrapperVersionsAreMigratable() {
        let current = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        let unversioned = current.replacingOccurrences(
            of: "# Muxy CLI wrapper version \(CLIWrapperScript.currentFormatVersion)",
            with: "# Muxy CLI wrapper. Resolves the bundled muxy-cli"
        )

        #expect(!CLIWrapperScript.requiresMigration(current))
        #expect(!CLIWrapperScript.requiresMigration(unversioned))
        #expect(CLIWrapperScript.requiresMigration(current, targetVersion: 2))
        #expect(CLIWrapperScript.requiresMigration(unversioned, targetVersion: 2))
    }

    @Test("migration updates legacy copies and preserves current and unrelated executables")
    @MainActor
    func migratesOnlyLegacyInstallations() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        let previousWrapper = CLIWrapperScript.contents(installedAppPath: "/Users/me/Downloads/Muxy.app")
        let legacy = """
        #!/bin/bash
        # Muxy CLI wrapper
        SOCKET="${MUXY_SOCKET_PATH:-muxy.sock}"
        """
        let unrelated = "#!/bin/bash\nprintf 'unrelated'\n"
        let installationPaths = ["/current", "/legacy", "/unrelated"]
        let contents = [
            "/current/muxy": previousWrapper,
            "/legacy/muxy": legacy,
            "/unrelated/muxy": unrelated,
        ]
        var installedPaths: [String] = []

        let failedPaths = CLIAccessor.migrateInstalledCLI(
            wrapper: wrapper,
            installationPaths: installationPaths,
            contentsAtPath: { contents[$0] },
            installWrapper: { installedWrapper, path in
                #expect(installedWrapper == wrapper)
                installedPaths.append(path)
                return true
            }
        )

        #expect(installedPaths == ["/legacy"])
        #expect(failedPaths.isEmpty)
    }

    @Test("migration reports legacy installations that cannot be written")
    @MainActor
    func reportsFailedMigrations() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        let legacy = """
        #!/bin/bash
        # Muxy CLI wrapper - Usage: muxy /path/to/project
        open "muxy://open?path=$1"
        """

        let installationPaths = ["/usr/local/bin", "/Users/me/bin", "/Users/me/.local/bin"]
        let failedPaths = CLIAccessor.migrateInstalledCLI(
            wrapper: wrapper,
            installationPaths: installationPaths,
            contentsAtPath: { _ in legacy },
            installWrapper: { _, _ in false }
        )

        #expect(failedPaths == installationPaths)
        #expect(CLIAccessor.migrationFailureLabels(failedPaths, home: "/Users/me") == [
            "/usr/local/bin/muxy",
            "~/bin/muxy",
            "~/.local/bin/muxy",
        ])
    }

    @Test("privileged installation uses fixed system tools and quotes wrapper contents")
    func privilegedInstallCommandIsHardened() {
        let wrapper = "#!/bin/bash\nprintf '%s' '\"$(touch /tmp/injected)\"'\n"
        let expected = "#!/bin/bash\nprintf 'legacy'\n"
        let command = CLIAccessor.adminInstallShellCommand(
            wrapper: wrapper,
            expectedInstalledContents: expected
        )

        #expect(command.contains("PATH=/usr/bin:/bin:/usr/sbin:/sbin; export PATH"))
        #expect(command.contains("umask 022"))
        #expect(command.contains("/bin/mkdir -p /usr/local/bin"))
        #expect(command.contains("/usr/bin/mktemp /usr/local/bin/.muxy.XXXXXX"))
        #expect(command.contains("/usr/bin/printf '%s' \(ShellEscaper.quote(wrapper))"))
        #expect(command.contains("/usr/bin/mktemp /usr/local/bin/.muxy.expected.XXXXXX"))
        #expect(command.contains("/usr/bin/printf '%s' \(ShellEscaper.quote(expected))"))
        #expect(command.contains("/usr/bin/cmp -s \"$expected\" /usr/local/bin/muxy"))
        #expect(command.contains("/bin/chmod 755"))
        #expect(command.contains("/bin/mv -f"))
        #expect(command.contains("/bin/rm -f"))
    }

    @Test("manual privileged installation does not require an existing wrapper")
    func manualPrivilegedInstallDoesNotCompareTarget() {
        let command = CLIAccessor.adminInstallShellCommand(wrapper: "wrapper")

        #expect(!command.contains(".muxy.expected."))
        #expect(!command.contains("/usr/bin/cmp"))
    }

    @Test("CLI update sheets require an unobstructed main window")
    @MainActor
    func cliUpdateSheetRequiresReadyMainWindow() {
        let mainWindow = NSWindow()
        let keyWindow = NSWindow()
        let existingSheet = NSWindow()

        #expect(CLIAccessor.readyWindow(keyWindow: keyWindow, mainWindow: mainWindow) === mainWindow)

        mainWindow.beginSheet(existingSheet)
        #expect(CLIAccessor.readyWindow(keyWindow: existingSheet, mainWindow: mainWindow) == nil)
        mainWindow.endSheet(existingSheet)

        #expect(CLIAccessor.readyWindow(keyWindow: keyWindow, mainWindow: nil) === keyWindow)
    }

    @Test("wrapper replacement stages executable content before replacing the target")
    @MainActor
    func wrapperReplacementIsAtomic() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("muxy-cli-wrapper-tests-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: directory) }
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let target = directory.appendingPathComponent("muxy")
        try Data("legacy".utf8).write(to: target)
        try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: target.path)

        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(CLIAccessor.writeWrapper(wrapper, to: directory.path))

        let saved = try String(contentsOf: target, encoding: .utf8)
        let attributes = try FileManager.default.attributesOfItem(atPath: target.path)
        let permissions = try #require(attributes[.posixPermissions] as? NSNumber)
        let remainingItems = try FileManager.default.contentsOfDirectory(atPath: directory.path)

        #expect(saved == wrapper)
        #expect(permissions.intValue == FilePermissions.executable)
        #expect(remainingItems == ["muxy"])
    }
}
