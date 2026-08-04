import Darwin
import MuxySessionProtocol

func argumentValue(_ name: String, in arguments: [String]) -> String? {
    guard let index = arguments.firstIndex(of: name), index + 1 < arguments.count else { return nil }
    return arguments[index + 1]
}

func currentDirectory() -> String {
    guard let raw = getcwd(nil, 0) else { return "/" }
    defer { free(raw) }
    return String(cString: raw)
}

func attachConfiguration() -> SessionAttachClient.Configuration? {
    guard let rawIdentifier = SessionProcessEnvironment.value("MUXY_SESSION_ID"),
          let identifier = SessionIdentifier(uuidString: rawIdentifier),
          let socketPath = SessionProcessEnvironment.value("MUXY_SESSION_SOCKET")
    else { return nil }

    return SessionAttachClient.Configuration(
        identifier: identifier,
        socketPath: socketPath,
        command: SessionProcessEnvironment.value("MUXY_SESSION_COMMAND") ?? "",
        shell: SessionProcessEnvironment.value("MUXY_SESSION_SHELL")
            ?? SessionProcessEnvironment.value("SHELL")
            ?? SessionShellIntegration.defaultShell,
        resourcesDirectory: SessionProcessEnvironment.value("MUXY_SESSION_RESOURCES")
            ?? SessionProcessEnvironment.value("GHOSTTY_RESOURCES_DIR")
            ?? "",
        workingDirectory: SessionProcessEnvironment.value("MUXY_SESSION_CWD") ?? currentDirectory(),
        metadata: attachMetadata()
    )
}

func attachMetadata() -> [SessionEnvironmentEntry] {
    let sources = [
        (SessionMetadataKey.project, "MUXY_SESSION_PROJECT"),
        (SessionMetadataKey.worktree, "MUXY_SESSION_WORKTREE"),
        (SessionMetadataKey.tab, "MUXY_SESSION_TAB"),
        (SessionMetadataKey.title, "MUXY_SESSION_TITLE"),
    ]
    return sources.compactMap { key, variable in
        guard let value = SessionProcessEnvironment.value(variable) else { return nil }
        return SessionEnvironmentEntry(key: key, value: value)
    }
}

func fail(_ message: String) -> Never {
    SessionLog.write(message)
    exit(2)
}

let arguments = CommandLine.arguments
guard arguments.count > 1 else {
    fail("usage: muxy-session daemon --socket <path> | muxy-session attach")
}

switch arguments[1] {
case "daemon":
    guard let socketPath = argumentValue("--socket", in: arguments) else {
        fail("usage: muxy-session daemon --socket <path>")
    }
    let idleTimeout = argumentValue("--idle-timeout", in: arguments)
        .flatMap(Int32.init) ?? SessionDaemon.idleTimeoutMilliseconds
    guard let daemon = SessionDaemon(socketPath: socketPath, idleTimeoutMilliseconds: idleTimeout) else {
        exit(0)
    }
    daemon.run()
    exit(0)
case "attach":
    guard let configuration = attachConfiguration() else {
        fail("muxy-session: MUXY_SESSION_ID and MUXY_SESSION_SOCKET are required")
    }
    exit(SessionAttachClient.run(configuration: configuration))
default:
    fail("muxy-session: unknown command \(arguments[1])")
}
