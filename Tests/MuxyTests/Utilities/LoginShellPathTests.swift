import Foundation
import Testing

@testable import Muxy

@Suite("LoginShellPath")
struct LoginShellPathTests {
    @Test("hydrate waits for resolved login shell PATH")
    func hydrateWaitsForResolvedLoginShellPath() async {
        let path = LoginShellPath()

        await path.hydrate {
            "/tmp/custom-bin:/usr/bin"
        }

        #expect(path.value == "/tmp/custom-bin:/usr/bin")
    }

    @Test("hydrate keeps default PATH when lookup fails")
    func hydrateKeepsDefaultPathWhenLookupFails() async {
        let path = LoginShellPath()

        await path.hydrate {
            nil
        }

        #expect(path.value == LoginShellPath.defaultPath)
    }

    @Test("environment hydration captures PATH and Copilot home together")
    func hydrateCapturesLoginShellEnvironment() async {
        let path = LoginShellPath()

        await path.hydrateEnvironment {
            LoginShellEnvironmentValues(
                path: "/tmp/custom-bin:/usr/bin",
                copilotHome: "/tmp/custom-copilot"
            )
        }

        #expect(path.value == "/tmp/custom-bin:/usr/bin")
        #expect(path.copilotHome == "/tmp/custom-copilot")
    }

    @Test("login shell lookup loads interactive configuration")
    func loginShellLookupLoadsInteractiveConfiguration() {
        #expect(LoginShellPath.shellArguments.prefix(3) == ["-l", "-i", "-c"])
        #expect(LoginShellPath.shellArguments.last?.contains("printenv COPILOT_HOME") == true)
    }

    @Test("login shell lookup extracts PATH without startup output")
    func loginShellLookupExtractsPathWithoutStartupOutput() {
        let output = """
        shell startup output
        __MUXY_PATH_START__/custom/bin:/usr/bin
        __MUXY_PATH_END__
        """

        #expect(LoginShellPath.extractPath(from: output) == "/custom/bin:/usr/bin")
    }

    @Test("login shell lookup extracts the complete environment")
    func loginShellLookupExtractsEnvironment() {
        let output = """
        shell startup output
        __MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__
        __MUXY_COPILOT_HOME_START__/custom/copilot__MUXY_COPILOT_HOME_END__
        """

        #expect(LoginShellPath.extractEnvironment(from: output) == LoginShellEnvironmentValues(
            path: "/custom/bin:/usr/bin",
            copilotHome: "/custom/copilot"
        ))
    }

    @Test("login shell lookup accepts an unset Copilot home")
    func loginShellLookupAcceptsUnsetCopilotHome() {
        let output = """
        __MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__
        __MUXY_COPILOT_HOME_START____MUXY_COPILOT_HOME_END__
        """

        #expect(LoginShellPath.extractEnvironment(from: output) == LoginShellEnvironmentValues(
            path: "/custom/bin:/usr/bin",
            copilotHome: nil
        ))
    }

    @Test("login shell lookup rejects malformed output")
    func loginShellLookupRejectsMalformedOutput() {
        #expect(LoginShellPath.extractPath(from: "/custom/bin:/usr/bin") == nil)
        #expect(LoginShellPath.extractPath(from: "__MUXY_PATH_START____MUXY_PATH_END__") == nil)
        #expect(LoginShellPath.extractEnvironment(
            from: "__MUXY_PATH_START__/usr/bin__MUXY_PATH_END__"
        ) == nil)
    }

    @Test("login shell lookup drains noisy startup output")
    func loginShellLookupDrainsNoisyStartupOutput() {
        let command = """
        /usr/bin/yes stdout | /usr/bin/head -c 400000
        /usr/bin/yes stderr | /usr/bin/head -c 400000 >&2
        printf '__MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__'
        """

        let path = LoginShellPath.readPath(
            shellPath: "/bin/sh",
            arguments: ["-c", command],
            timeout: .seconds(30)
        )

        #expect(path == "/custom/bin:/usr/bin")
    }

    @Test("login shell lookup does not wait for descendants holding output pipes")
    func loginShellLookupDoesNotWaitForDescendantOutput() {
        let command = """
        /bin/sleep 1 &
        printf '__MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__'
        """

        let path = LoginShellPath.readPath(
            shellPath: "/bin/sh",
            arguments: ["-c", command],
            timeout: .milliseconds(100)
        )

        #expect(path == nil)
    }

    @Test("login shell lookup tolerates a truncated UTF-8 scalar before PATH")
    func loginShellLookupToleratesTruncatedUTF8() {
        let output = Data([0x80]) + Data(
            "__MUXY_PATH_START__/custom/bin:/usr/bin__MUXY_PATH_END__".utf8
        )

        #expect(LoginShellPath.extractPath(from: output) == "/custom/bin:/usr/bin")
    }
}
