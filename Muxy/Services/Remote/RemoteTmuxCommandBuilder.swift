import Foundation

enum RemoteTmuxCommandBuilder {
    static func availabilityCommand() -> String {
        "command -v tmux >/dev/null 2>&1 && tmux -V >/dev/null 2>&1 "
            + "&& printf '%s\\n' \(ShellEscaper.escape(RemoteTmuxSessionService.availableMarker)) "
            + "|| printf '%s\\n' \(ShellEscaper.escape(RemoteTmuxSessionService.unavailableMarker))"
    }

    static func hasSessionCommand(for session: RemoteTmuxSession) -> String {
        let target = ShellEscaper.escape(session.target)
        let unavailable = ShellEscaper.escape(RemoteTmuxSessionService.unavailableMarker)
        let present = ShellEscaper.escape(RemoteTmuxSessionService.presentMarker)
        let absent = ShellEscaper.escape(RemoteTmuxSessionService.absentMarker)
        let unknown = ShellEscaper.escape(RemoteTmuxSessionService.unknownMarker)
        let name = ShellEscaper.escape(session.name)
        return "if ! command -v tmux >/dev/null 2>&1; then printf '%s\\n' \(unavailable); "
            + "else __muxy_tmux_error=$(LC_ALL=C tmux has-session -t \(target) 2>&1); __muxy_tmux_status=$?; "
            + "if [ \"$__muxy_tmux_status\" -eq 0 ]; then printf '%s\\n' \(present); "
            + "else case \"$__muxy_tmux_error\" in \"can't find session: \(name)\") "
            + "printf '%s\\n' \(absent) ;; *) printf '%s\\n' \(unknown) ;; esac; fi; fi"
    }

    static func attachOrCreateCommand(
        for session: RemoteTmuxSession,
        initialCommand: String,
        createIfMissing: Bool = true
    ) -> String {
        let target = ShellEscaper.escape(session.target)
        let name = ShellEscaper.escape(session.name)
        var creationArguments = ["tmux new-session -d -s \(name)"]
        creationArguments.append(ShellEscaper.escape(initialCommand))
        let create = creationArguments.joined(separator: " ")
        let createIfNeeded = createIfMissing
            ? tombstoneSetup(for: session)
            + "if ! tmux has-session -t \(target) 2>/dev/null; then "
            + "[ ! -e \"$__muxy_tmux_tombstone\" ] || exit 1; \(create) >/dev/null 2>&1 || :; "
            + "if [ -e \"$__muxy_tmux_tombstone\" ]; then "
            + "tmux kill-session -t \(target) >/dev/null 2>&1 || :; exit 1; fi; fi; "
            : ""
        return "command -v tmux >/dev/null 2>&1 || exit 127; "
            + createIfNeeded
            + "tmux has-session -t \(target) 2>/dev/null || exit 1; "
            + "exec tmux attach-session -d -t \(target)"
    }

    static func killSessionCommand(for session: RemoteTmuxSession) -> String {
        let target = ShellEscaper.escape(session.target)
        return tombstoneSetup(for: session)
            + "if ! (umask 077 && mkdir \"$__muxy_tmux_tombstone\") 2>/dev/null; then "
            + "[ -d \"$__muxy_tmux_tombstone\" ] && [ ! -L \"$__muxy_tmux_tombstone\" ] || exit 1; fi; "
            + "command -v tmux >/dev/null 2>&1 || exit 127; "
            + "tmux kill-session -t \(target) >/dev/null 2>&1"
    }

    private static func tombstoneSetup(for session: RemoteTmuxSession) -> String {
        "[ -n \"$HOME\" ] || exit 1; __muxy_tmux_state_dir=\"$HOME/.muxy/tmux\"; "
            + "(umask 077 && mkdir -p \"$__muxy_tmux_state_dir\") || exit 1; "
            + "__muxy_tmux_tombstone=\"$__muxy_tmux_state_dir/\(session.name).closed\"; "
    }
}
