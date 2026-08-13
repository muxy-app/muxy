import Foundation
import Testing

@testable import Muxy

@Suite("SSH invocation parsing")
struct SSHInvocationParserTests {
    @Test("reads a bare host")
    func bareHost() throws {
        let destination = try #require(SSHInvocationParser.destination(arguments: ["example.com"]))

        #expect(destination.host == "example.com")
        #expect(destination.user == nil)
        #expect(destination.port == nil)
    }

    @Test("reads user@host")
    func userAtHost() throws {
        let destination = try #require(SSHInvocationParser.destination(arguments: ["deploy@example.com"]))

        #expect(destination.host == "example.com")
        #expect(destination.user == "deploy")
        #expect(destination.target == "deploy@example.com")
    }

    @Test("reads a separated option value")
    func separatedOptionValue() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["-p", "2222", "-l", "deploy", "example.com"]
        ))

        #expect(destination.host == "example.com")
        #expect(destination.port == 2222)
        #expect(destination.user == "deploy")
    }

    @Test("reads an attached option value")
    func attachedOptionValue() throws {
        let destination = try #require(SSHInvocationParser.destination(arguments: ["-p2222", "example.com"]))

        #expect(destination.port == 2222)
        #expect(destination.host == "example.com")
    }

    @Test("skips clustered valueless flags")
    func clusteredFlags() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["-tt", "-vvv", "-C", "example.com"]
        ))

        #expect(destination.host == "example.com")
    }

    @Test("takes the value of a trailing flag in a cluster")
    func clusteredFlagWithValue() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["-tp", "2200", "example.com"]
        ))

        #expect(destination.port == 2200)
        #expect(destination.host == "example.com")
    }

    @Test("keeps an identity file")
    func identityFile() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["-i", "/Users/me/.ssh/id_ed25519", "example.com"]
        ))

        #expect(destination.identityFile == "/Users/me/.ssh/id_ed25519")
        #expect(destination.connectionArguments.contains("IdentitiesOnly=yes"))
    }

    @Test("refuses a remote command after the destination")
    func remoteCommandRefused() {
        #expect(SSHInvocationParser.destination(
            arguments: ["example.com", "sudo", "-p", "9999", "reboot"]
        ) == nil)
        #expect(SSHInvocationParser.destination(
            arguments: ["-t", "bastion", "ssh", "target"]
        ) == nil)
    }

    @Test("reads an ssh URI")
    func sshURI() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["ssh://deploy@example.com:2022"]
        ))

        #expect(destination.host == "example.com")
        #expect(destination.user == "deploy")
        #expect(destination.port == 2022)
    }

    @Test("keeps a config alias verbatim so ssh_config still resolves it")
    func configAlias() throws {
        let destination = try #require(SSHInvocationParser.destination(arguments: ["my-server"]))

        #expect(destination.host == "my-server")
    }

    @Test("refuses unsupported value-taking options")
    func refusesUnreproducible() {
        #expect(SSHInvocationParser.destination(arguments: ["-J", "jump", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-F", "/tmp/other", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-W", "host:80", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "ProxyJump=jump", "example.com"]
        ) == nil)
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "ProxyCommand=nc %h %p", "example.com"]
        ) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-P", "production", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-I", "/tmp/provider", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-4", "example.com"]) == nil)
    }

    @Test("refuses an invocation with no destination")
    func refusesMissingDestination() {
        #expect(SSHInvocationParser.destination(arguments: []) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-v"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-p"]) == nil)
    }

    @Test("refuses a malformed port")
    func refusesBadPort() {
        #expect(SSHInvocationParser.destination(arguments: ["-p", "abc", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-p", "0", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-p", "70000", "example.com"]) == nil)
    }

    @Test("refuses unrepresented -o options")
    func refusesUnrepresentedOption() {
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "ServerAliveInterval=30", "example.com"]
        ) == nil)
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "CanonicalizeHostname=yes", "example.com"]
        ) == nil)
    }

    @Test("applies -o values that change where the upload connects")
    func appliesRoutingOptions() throws {
        let destination = try #require(SSHInvocationParser.destination(arguments: [
            "-o", "Port=2222",
            "-o", "User=deploy",
            "-o", "IdentityFile=/Users/me/.ssh/id_ed25519",
            "example.com",
        ]))

        #expect(destination.port == 2222)
        #expect(destination.user == "deploy")
        #expect(destination.identityFile == "/Users/me/.ssh/id_ed25519")
    }

    @Test("matches ssh precedence where the first value of a repeated option wins")
    func firstValueWins() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["-p", "2222", "-o", "Port=3333", "example.com"]
        ))

        #expect(destination.port == 2222)
    }

    @Test("refuses multiple additive identity files")
    func refusesMultipleIdentityFiles() {
        #expect(SSHInvocationParser.destination(
            arguments: ["-i", "/tmp/first", "-i", "/tmp/second", "example.com"]
        ) == nil)
        #expect(SSHInvocationParser.destination(arguments: [
            "-o", "IdentityFile=/tmp/first",
            "-o", "IdentityFile=/tmp/second",
            "example.com",
        ]) == nil)
    }

    @Test("resolves a relative identity from the ssh process working directory")
    func resolvesRelativeIdentity() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["-i", "keys/../keys/id_ed25519", "example.com"],
            workingDirectory: "/Users/example/project"
        ))

        #expect(destination.identityFile == "/Users/example/project/keys/../keys/id_ed25519")
        #expect(SSHInvocationParser.destination(
            arguments: ["-i", "keys/id_ed25519", "example.com"]
        ) == nil)
    }

    @Test("refuses identity expansion that cannot be reconstructed faithfully")
    func refusesExpandedIdentity() {
        #expect(SSHInvocationParser.destination(
            arguments: ["-i", "%d/.ssh/id_ed25519", "example.com"],
            workingDirectory: "/Users/example/project"
        ) == nil)
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "IdentityFile=${SSH_KEYS}/id_ed25519", "example.com"],
            workingDirectory: "/Users/example/project"
        ) == nil)
    }

    @Test("user@host still overrides an earlier login option")
    func targetUserOverridesOption() throws {
        let destination = try #require(SSHInvocationParser.destination(
            arguments: ["-l", "ignored", "deploy@example.com"]
        ))

        #expect(destination.user == "deploy")
    }

    @Test("refuses whitespace separated options the same as equals separated ones")
    func refusesWhitespaceSeparatedOption() {
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "ProxyJump jump", "example.com"]
        ) == nil)
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "ProxyCommand nc %h %p", "example.com"]
        ) == nil)
    }

    @Test("refuses an option that rewrites the host behind an alias")
    func refusesHostNameOverride() {
        #expect(SSHInvocationParser.destination(
            arguments: ["-o", "HostName=real.example.com", "alias"]
        ) == nil)
    }

    @Test("refuses a malformed or empty -o value")
    func refusesMalformedOption() {
        #expect(SSHInvocationParser.destination(arguments: ["-o", "NoSeparator", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-o", "Port=abc", "example.com"]) == nil)
        #expect(SSHInvocationParser.destination(arguments: ["-o", "User=", "example.com"]) == nil)
    }

    @Test("only matches an ssh executable")
    func onlySSHExecutable() {
        #expect(SSHInvocationParser.destination(from: ProcessInvocation(
            executablePath: "/usr/bin/ssh",
            arguments: ["ssh", "example.com"]
        ))?.host == "example.com")

        #expect(SSHInvocationParser.destination(from: ProcessInvocation(
            executablePath: "/bin/zsh",
            arguments: ["zsh", "example.com"]
        )) == nil)

        #expect(SSHInvocationParser.destination(from: ProcessInvocation(
            executablePath: "/usr/bin/sshfs",
            arguments: ["sshfs", "example.com"]
        )) == nil)
    }
}
