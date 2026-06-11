import Foundation

struct ResolvedLaunch: Equatable {
    let executable: String
    let arguments: [String]
    let workingDirectory: String?
}

enum CommandTransform {
    static func resolve(
        executable: String,
        arguments: [String],
        workingDirectory: String?,
        environment: [String: String]? = nil,
        in context: WorkspaceContext
    ) -> ResolvedLaunch {
        guard case let .ssh(destination) = context else {
            return ResolvedLaunch(
                executable: executable,
                arguments: arguments,
                workingDirectory: workingDirectory
            )
        }
        let mergedEnvironment = SSHEnvironmentVariables.merged(device: destination.environment, command: environment)
        let remoteCommand = RemoteCommandBuilder.remoteCommand(
            executable: executable,
            arguments: arguments,
            workingDirectory: workingDirectory,
            environment: mergedEnvironment
        )
        return sshLaunch(destination: destination, tty: false, remoteCommand: remoteCommand)
    }

    static func resolveShell(
        shellCommand: String,
        workingDirectory: String?,
        environment: [String: String]? = nil,
        in context: WorkspaceContext
    ) -> ResolvedLaunch {
        guard case let .ssh(destination) = context else {
            return ResolvedLaunch(
                executable: "/bin/sh",
                arguments: ["-c", shellCommand],
                workingDirectory: workingDirectory
            )
        }
        let mergedEnvironment = SSHEnvironmentVariables.merged(device: destination.environment, command: environment)
        let remoteCommand = RemoteCommandBuilder.remoteShellCommand(
            shell: shellCommand,
            workingDirectory: workingDirectory,
            environment: mergedEnvironment
        )
        return sshLaunch(destination: destination, tty: false, remoteCommand: remoteCommand)
    }

    private static func sshLaunch(destination: SSHDestination, tty: Bool, remoteCommand: String) -> ResolvedLaunch {
        let options = tty ? SSHDestination.terminalOptions : SSHDestination.batchOptions
        let ttyFlag = tty ? ["-tt"] : ["-T"]
        return ResolvedLaunch(
            executable: "/usr/bin/ssh",
            arguments: destination.connectionArguments + options + ttyFlag + [destination.target, "--", remoteCommand],
            workingDirectory: nil
        )
    }
}
