import Foundation
import Testing

@testable import Muxy

@Suite("CLIWrapperScript")
struct CLIWrapperScriptTests {
    @Test("wrapper execs the bundled script rather than embedding the CLI body")
    func wrapperExecsBundledScript() {
        let wrapper = CLIWrapperScript.contents(installedAppPath: "/Applications/Muxy.app")
        #expect(wrapper.hasPrefix("#!/bin/bash"))
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

        let failedPaths = CLIAccessor.migrateInstalledCLI(
            wrapper: wrapper,
            installationPaths: ["/usr/local/bin"],
            contentsAtPath: { _ in legacy },
            installWrapper: { _, _ in false }
        )

        #expect(failedPaths == ["/usr/local/bin"])
    }
}
