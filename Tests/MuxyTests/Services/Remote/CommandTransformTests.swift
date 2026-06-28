import Foundation
import Testing

@testable import Muxy

@Suite("SSHDestination connection arguments")
struct SSHDestinationTests {
    @Test("bare host defers everything to ssh config")
    func bareHost() {
        let destination = SSHDestination(host: "prod")
        #expect(destination.target == "prod")
        #expect(destination.connectionArguments.isEmpty)
    }

    @Test("user is folded into the target")
    func userTarget() {
        let destination = SSHDestination(host: "1.2.3.4", user: "deploy")
        #expect(destination.target == "deploy@1.2.3.4")
    }

    @Test("port and identity file become ssh options")
    func portAndIdentity() {
        let destination = SSHDestination(host: "prod", port: 2222, identityFile: "~/.ssh/id_ed25519")
        #expect(destination.connectionArguments == ["-p", "2222", "-i", "~/.ssh/id_ed25519", "-o", "IdentitiesOnly=yes"])
    }

    @Test("empty advanced fields are dropped")
    func emptyFieldsDropped() {
        let destination = SSHDestination(host: "prod", user: "", identityFile: "")
        #expect(destination.user == nil)
        #expect(destination.identityFile == nil)
        #expect(destination.connectionArguments.isEmpty)
    }

    @Test("ssh workspace data round-trips advanced fields")
    func dataRoundTrip() throws {
        let data = SSHWorkspaceData(
            host: "prod",
            remoteRoot: "~/code",
            port: 2200,
            user: "ci",
            identityFile: "~/k",
            environment: ["TERM": "screen-256color", "LANG": "C.UTF-8"]
        )
        let decoded = try JSONDecoder().decode(SSHWorkspaceData.self, from: JSONEncoder().encode(data))
        #expect(decoded == data)
        #expect(decoded.destination.target == "ci@prod")
        #expect(decoded.destination.connectionArguments.contains("2200"))
        #expect(decoded.destination.environment["TERM"] == "screen-256color")
    }

    @Test("legacy workspace data defaults TERM")
    func legacyDataDefaultsTerm() throws {
        let decoded = try JSONDecoder().decode(
            SSHWorkspaceData.self,
            from: Data(#"{"host":"prod","remoteRoot":"~"}"#.utf8)
        )
        #expect(decoded.environment == SSHEnvironmentVariables.default)
    }
}

@Suite("CommandTransform routing")
struct CommandTransformTests {
    private let destination = SSHDestination(host: "prod", remoteRoot: "~/code")

    @Test("local context is identity")
    func localIsIdentity() {
        let resolved = CommandTransform.resolve(
            executable: "/usr/bin/env",
            arguments: ["git", "status"],
            workingDirectory: "/Users/me/proj",
            in: .local
        )
        #expect(resolved.executable == "/usr/bin/env")
        #expect(resolved.arguments == ["git", "status"])
        #expect(resolved.workingDirectory == "/Users/me/proj")
    }

    @Test("ssh context wraps git as a non-tty remote command")
    func sshWrapsGit() {
        let resolved = CommandTransform.resolve(
            executable: "/usr/bin/env",
            arguments: ["git", "-C", "~/code/api", "status"],
            workingDirectory: nil,
            in: .ssh(destination)
        )
        #expect(resolved.executable == "/usr/bin/ssh")
        #expect(resolved.workingDirectory == nil)
        #expect(resolved.arguments.contains("-T"))
        #expect(resolved.arguments.contains("prod"))
        #expect(resolved.arguments.last == "export TERM=xterm-256color; /usr/bin/env git -C ~/code/api status")
    }

    @Test("ssh folds working directory into the remote command")
    func sshFoldsWorkingDirectory() {
        let resolved = CommandTransform.resolve(
            executable: "npm",
            arguments: ["run", "build"],
            workingDirectory: "~/code/api",
            in: .ssh(destination)
        )
        #expect(resolved.arguments.last == "export TERM=xterm-256color; cd ~/code/api && npm run build")
    }

    @Test("ssh exports environment before the command")
    func sshExportsEnvironment() {
        let resolved = CommandTransform.resolve(
            executable: "make",
            arguments: [],
            workingDirectory: "~/code/api",
            environment: ["CI": "1", "TOKEN": "a b"],
            in: .ssh(destination)
        )
        #expect(resolved.arguments.last == "export CI=1; export TERM=xterm-256color; export TOKEN='a b'; cd ~/code/api && make")
    }

    @Test("command environment overrides device environment")
    func commandEnvironmentOverridesDeviceEnvironment() {
        let destination = SSHDestination(host: "prod", environment: ["TERM": "screen-256color", "LANG": "C.UTF-8"])
        let resolved = CommandTransform.resolve(
            executable: "env",
            arguments: [],
            workingDirectory: nil,
            environment: ["TERM": "vt100"],
            in: .ssh(destination)
        )
        #expect(resolved.arguments.last == "export LANG=C.UTF-8; export TERM=vt100; env")
    }

    @Test("shell strings are wrapped, not re-escaped")
    func sshShellOpaque() {
        let resolved = CommandTransform.resolveShell(
            shellCommand: "echo $HOME && ls -la",
            workingDirectory: "~/code/api",
            in: .ssh(destination)
        )
        #expect(resolved.arguments.last == "export TERM=xterm-256color; cd ~/code/api && ( echo $HOME && ls -la )")
    }

    @Test("local shell uses /bin/sh -c")
    func localShell() {
        let resolved = CommandTransform.resolveShell(
            shellCommand: "echo hi",
            workingDirectory: "/tmp",
            in: .local
        )
        #expect(resolved.executable == "/bin/sh")
        #expect(resolved.arguments == ["-c", "echo hi"])
        #expect(resolved.workingDirectory == "/tmp")
    }
}

@Suite("RemoteCommandBuilder quoting")
struct RemoteCommandBuilderTests {
    @Test("leading tilde is preserved for remote expansion")
    func tildePreserved() {
        #expect(RemoteCommandBuilder.quoteRemotePath("~") == "~")
        #expect(RemoteCommandBuilder.quoteRemotePath("~/code/api") == "~/code/api")
    }

    @Test("tilde path with spaces keeps tilde bare and escapes the rest")
    func tildeWithSpaces() {
        #expect(RemoteCommandBuilder.quoteRemotePath("~/My Proj") == "~/'My Proj'")
    }

    @Test("absolute path with metacharacters is fully quoted")
    func absoluteQuoted() {
        #expect(RemoteCommandBuilder.quoteRemotePath("/a b/c") == "'/a b/c'")
        #expect(RemoteCommandBuilder.quoteRemotePath("/plain/path") == "/plain/path")
    }

    @Test("embedded single quotes are escaped")
    func embeddedSingleQuotes() {
        #expect(RemoteCommandBuilder.quoteRemotePath("/it's here") == "'/it'\\''s here'")
    }

    @Test("dangerous tokens are quoted in the command")
    func dangerousTokens() {
        let command = RemoteCommandBuilder.remoteCommand(
            executable: "echo",
            arguments: ["a; rm -rf /", "$(whoami)", "a|b"],
            workingDirectory: nil
        )
        #expect(command == "echo 'a; rm -rf /' '$(whoami)' 'a|b'")
    }
}

@Suite("SSHDestination option-injection hardening")
struct SSHDestinationHardeningTests {
    @Test("leading-dash host is sanitized so ssh cannot parse it as an option")
    func sanitizesDashHost() {
        let destination = SSHDestination(host: "-oProxyCommand=touch /tmp/pwned")
        #expect(!destination.host.hasPrefix("-"))
        #expect(!destination.target.hasPrefix("-"))
    }

    @Test("leading-dash user is sanitized")
    func sanitizesDashUser() {
        let destination = SSHDestination(host: "prod", user: "-oProxyCommand=x")
        #expect(destination.user?.hasPrefix("-") == false)
    }

    @Test("decoded destinations are sanitized too")
    func sanitizesDecodedHost() throws {
        let json = #"{"host":"-E/tmp/log","remoteRoot":"~"}"#
        let decoded = try JSONDecoder().decode(SSHDestination.self, from: Data(json.utf8))
        #expect(!decoded.host.hasPrefix("-"))
    }

    @Test("host validity check rejects empty and leading-dash hosts")
    func validityCheck() {
        #expect(SSHDestination.isValidHost("prod"))
        #expect(!SSHDestination.isValidHost(""))
        #expect(!SSHDestination.isValidHost("-oProxyCommand=x"))
    }
}

@Suite("RemoteCommandBuilder environment hardening")
struct RemoteEnvironmentTests {
    @Test("invalid environment keys are dropped")
    func dropsInvalidKeys() {
        let prefix = RemoteCommandBuilder.environmentPrefix([
            "GOOD": "1",
            "BAD KEY": "x",
            "BAD\nKEY": "x",
            "9LEADING": "x",
        ])
        #expect(prefix == "export GOOD=1; ")
    }

    @Test("values are quoted, valid keys are not")
    func quotesValuesNotKeys() {
        let prefix = RemoteCommandBuilder.environmentPrefix(["TOKEN": "a b; rm -rf /"])
        #expect(prefix == "export TOKEN='a b; rm -rf /'; ")
    }
}

@Suite("SSH environment text")
struct SSHEnvironmentTextTests {
    @Test("formats environment in stable key order")
    func formatsEnvironment() {
        let text = SSHEnvironmentText.format(["TERM": "xterm-256color", "LANG": "C.UTF-8"])
        #expect(text == "LANG=C.UTF-8\nTERM=xterm-256color")
    }

    @Test("parses KEY value lines")
    func parsesEnvironmentText() throws {
        let environment = try SSHEnvironmentText.parse("""
        TERM=xterm-256color
        LANG=C.UTF-8
        EMPTY=
        """).get()

        #expect(environment["TERM"] == "xterm-256color")
        #expect(environment["LANG"] == "C.UTF-8")
        #expect(environment["EMPTY"] == "")
    }

    @Test("preserves environment value whitespace")
    func preservesEnvironmentValueWhitespace() throws {
        let environment = try SSHEnvironmentText.parse("TOKEN= value ").get()

        #expect(environment["TOKEN"] == " value ")
    }

    @Test("rejects malformed environment lines")
    func rejectsMalformedEnvironmentText() {
        #expect(SSHEnvironmentText.parse("TERM xterm").failure == .missingAssignment(line: 1))
        #expect(SSHEnvironmentText.parse("BAD KEY=x").failure == .invalidKey("BAD KEY"))
        #expect(SSHEnvironmentText.parse("TERM=x\nTERM=y").failure == .duplicateKey("TERM"))
    }
}

private extension Result where Success == [String: String], Failure == SSHEnvironmentTextError {
    var failure: SSHEnvironmentTextError? {
        if case let .failure(error) = self { return error }
        return nil
    }
}

@Suite("SSHDestination connection identity")
struct SSHConnectionKeyTests {
    @Test("connection key ignores the remote root")
    func ignoresRemoteRoot() {
        let a = SSHDestination(host: "prod", remoteRoot: "~/code")
        let b = SSHDestination(host: "prod", remoteRoot: "~/other")
        #expect(a.connectionKey == b.connectionKey)
        #expect(a != b)
    }

    @Test("connection key ignores environment")
    func ignoresEnvironment() {
        let a = SSHDestination(host: "prod", environment: ["TERM": "xterm-256color"])
        let b = SSHDestination(host: "prod", environment: ["TERM": "screen-256color"])
        #expect(a.connectionKey == b.connectionKey)
        #expect(a != b)
    }

    @Test("connection key distinguishes transport fields")
    func distinguishesTransport() {
        let base = SSHDestination(host: "prod", remoteRoot: "~", port: 22, user: "ci")
        #expect(base.connectionKey != SSHDestination(host: "prod", port: 2222, user: "ci").connectionKey)
        #expect(base.connectionKey != SSHDestination(host: "prod", port: 22, user: "other").connectionKey)
        #expect(base.connectionKey != SSHDestination(host: "other", port: 22, user: "ci").connectionKey)
    }
}

@Suite("RemoteCommandBuilder containment guard")
struct RemoteContainmentGuardTests {
    @Test("guard resolves real paths and aborts when the target escapes the root")
    func guardsEscape() {
        let prefix = RemoteCommandBuilder.containmentGuardPrefix(root: "/srv/app", target: "/srv/app/link")
        #expect(prefix.contains("pwd -P"))
        #expect(prefix.contains("exit \(RemoteCommandBuilder.containmentEscapeExitCode)"))
        #expect(prefix.contains("\"$__muxy_root\"/*"))
    }

    @Test("guard quotes the root and target")
    func guardQuotesPaths() {
        let prefix = RemoteCommandBuilder.containmentGuardPrefix(root: "/a b", target: "/a b/c")
        #expect(prefix.contains("'/a b'"))
        #expect(prefix.contains("'/a b/c'"))
    }
}

@Suite("SSHFieldSanitizer")
struct SSHFieldSanitizerTests {
    @Test("leading dashes are stripped from host and optional arguments")
    func stripsLeadingDashes() {
        #expect(SSHFieldSanitizer.host(" -oProxyCommand=x ") == "oProxyCommand=x")
        #expect(SSHFieldSanitizer.optionalArgument("--bad") == "bad")
    }

    @Test("empty optional arguments become nil")
    func emptyBecomesNil() {
        #expect(SSHFieldSanitizer.optionalArgument("   ") == nil)
        #expect(SSHFieldSanitizer.identityFile("") == nil)
    }

    @Test("empty root defaults to tilde")
    func rootDefaults() {
        #expect(SSHFieldSanitizer.root(nil) == "~")
        #expect(SSHFieldSanitizer.root("  ") == "~")
        #expect(SSHFieldSanitizer.root("~/code") == "~/code")
    }

    @Test("stored workspace data sanitizes a leading-dash host")
    func workspaceDataSanitizes() throws {
        let data = SSHWorkspaceData(host: "-E/tmp/log", user: "-x")
        #expect(!data.host.hasPrefix("-"))
        #expect(data.user?.hasPrefix("-") == false)
        let decoded = try JSONDecoder().decode(
            SSHWorkspaceData.self,
            from: Data(#"{"host":"-E/tmp/log","user":"-x"}"#.utf8)
        )
        #expect(!decoded.host.hasPrefix("-"))
        #expect(decoded.user?.hasPrefix("-") == false)
    }
}
