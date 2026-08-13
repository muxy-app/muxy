enum RemoteLoginShellCommand {
    static func bootstrap(_ script: String) -> String {
        "exec /bin/sh -c \(ShellEscaper.escape(script))"
    }

    static func dispatch(
        posixScript: String,
        fishScript: String,
        interactive: Bool
    ) -> String {
        let flags = interactive ? "-l -i" : "-l"
        let escapedPosixScript = ShellEscaper.escape(posixScript)
        let escapedFishScript = ShellEscaper.escape(fishScript)
        let fishBranch = "fish) exec \"$__muxy_shell\" \(flags) -c \(escapedFishScript) \"$__muxy_shell\""
        let posixBranch = "*) exec \"$__muxy_shell\" \(flags) -c \(escapedPosixScript) \"$__muxy_shell\""
        return [
            "__muxy_shell=${SHELL:-/bin/sh}",
            "__muxy_shell_name=${__muxy_shell##*/}",
            "case \"$__muxy_shell_name\" in \(fishBranch) ;; \(posixBranch) ;; esac",
        ].joined(separator: "; ")
    }
}
