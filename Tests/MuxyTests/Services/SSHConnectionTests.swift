import Foundation
import Testing

@testable import Muxy
import MuxySSH

@Suite("SSH connection configuration")
struct SSHConnectionTests {
    @Test("legacy workspace data infers private-key auth when identity file exists")
    func legacyIdentityFileInfersPrivateKeyAuth() throws {
        let decoded = try JSONDecoder().decode(
            SSHWorkspaceData.self,
            from: Data(#"{"host":"prod","remoteRoot":"~/code","identityFile":"~/.ssh/id_ed25519"}"#.utf8)
        )

        #expect(decoded.authenticationMethod == .privateKey)
    }

    @Test("password auth pulls its secret from keychain")
    func passwordAuthUsesKeychain() {
        let host = "muxy-test-\(UUID().uuidString)"
        let user = "deploy"
        let password = "secret-\(UUID().uuidString)"
        KeychainSSHHelper.storePassword(password, host: host, user: user, port: 2201)
        defer {
            KeychainSSHHelper.deletePassword(host: host, user: user, port: 2201)
        }

        let configuration = SSHConnectionConfiguration.make(
            destination: SSHDestination(
                host: host,
                remoteRoot: "/srv/app",
                port: 2201,
                user: user,
                authenticationMethod: .password
            )
        )

        #expect(configuration.authentication == .password(password))
        #expect(configuration.initialShellInput == "cd /srv/app\n")
    }

    @Test("private key auth uses the selected identity file")
    func privateKeyAuthUsesIdentityFile() {
        let configuration = SSHConnectionConfiguration.make(
            destination: SSHDestination(
                host: "prod",
                remoteRoot: "/srv/app path",
                user: "deploy",
                identityFile: "~/.ssh/id_ed25519",
                authenticationMethod: .privateKey
            ),
            command: "swift test"
        )

        #expect(configuration.authentication == .privateKey(path: "~/.ssh/id_ed25519"))
        #expect(configuration.remoteExecCommand == "cd '/srv/app path'; swift test")
        #expect(configuration.initialShellInput == "cd '/srv/app path'\n")
    }

    @Test("remote ssh panes use a local surface working directory")
    func remoteSurfaceUsesLocalWorkingDirectory() {
        let configuration = SSHConnectionConfiguration.make(
            destination: SSHDestination(
                host: "prod",
                remoteRoot: "/srv/app",
                user: "deploy"
            )
        )

        #expect(configuration.localSurfaceWorkingDirectory == NSHomeDirectory())
    }

    @Test("legacy ssh cli startup command is discarded for native ssh")
    @MainActor
    func legacySSHCLIStartupCommandIsDiscarded() {
        let pane = TerminalPaneState(
            projectPath: "/tmp/project",
            startupCommand: "/usr/bin/ssh prod -- 'echo hi'"
        )

        #expect(pane.migratedSSHStartupCommand == nil)
    }

    @Test("ordinary startup command is preserved for native ssh")
    @MainActor
    func ordinarySSHStartupCommandIsPreserved() {
        let pane = TerminalPaneState(
            projectPath: "/tmp/project",
            startupCommand: "echo hi"
        )

        #expect(pane.migratedSSHStartupCommand == "echo hi")
    }

    @Test("remote pane restore bypasses local startup command launch")
    @MainActor
    func remotePaneRestoreLaunch() {
        let pane = TerminalPaneState(
            projectPath: "/tmp/project",
            startupCommand: "echo hi",
            startupCommandInteractive: true,
            closesOnStartupCommandExit: true,
            sshConfiguration: SSHConnectionConfiguration.make(
                destination: SSHDestination(
                    host: "prod",
                    remoteRoot: "/srv/app",
                    user: "deploy",
                    identityFile: "~/.ssh/id_ed25519",
                    authenticationMethod: .privateKey
                )
            )
        )

        let launch = pane.consumeRestoredLaunch()

        #expect(launch.command == nil)
        #expect(launch.interactive == false)
        #expect(launch.closesOnCommandExit == false)
    }
}
