import Foundation

enum RemoteCommandBuilder {
    static func quoteRemotePath(_ path: String) -> String {
        guard path == "~" || path.hasPrefix("~/") else {
            return ShellEscaper.escape(path)
        }
        guard path != "~" else { return "~" }
        let remainder = String(path.dropFirst(2))
        return "~/" + ShellEscaper.escape(remainder)
    }

    static func changeDirectoryPrefix(_ workingDirectory: String?) -> String {
        guard let workingDirectory, !workingDirectory.isEmpty else { return "" }
        return "cd \(quoteRemotePath(workingDirectory)) && "
    }

    static let containmentEscapeExitCode = 9

    static func containmentGuardPrefix(root: String, target: String) -> String {
        let quotedRoot = quoteRemotePath(root)
        let quotedTarget = quoteRemotePath(target)
        return "__muxy_root=$(cd \(quotedRoot) 2>/dev/null && { pwd -P; printf '\\001'; }) "
            + "|| exit \(containmentEscapeExitCode); "
            + "__muxy_root=${__muxy_root%?}; __muxy_root=${__muxy_root%?}; "
            + "__muxy_resolve() { "
            + "__muxy_r_path=$1; __muxy_r_suffix=; __muxy_r_hops=0; "
            + "case \"$__muxy_r_path\" in /*) ;; *) __muxy_r_path=\"$PWD/$__muxy_r_path\" ;; esac; "
            + "while :; do "
            + "while [ ! -e \"$__muxy_r_path\" ] && [ ! -L \"$__muxy_r_path\" ]; do "
            + "[ \"$__muxy_r_path\" != / ] || return 1; "
            + "__muxy_r_name=${__muxy_r_path##*/}; __muxy_r_parent=${__muxy_r_path%/*}; "
            + "[ -n \"$__muxy_r_parent\" ] || __muxy_r_parent=/; "
            + "__muxy_r_suffix=\"/$__muxy_r_name$__muxy_r_suffix\"; __muxy_r_path=$__muxy_r_parent; "
            + "done; "
            + "if [ -L \"$__muxy_r_path\" ]; then "
            + "__muxy_r_link=$(readlink -n \"$__muxy_r_path\" && printf '\\001') || return 1; "
            + "__muxy_r_link=${__muxy_r_link%?}; "
            + "__muxy_r_hops=$((__muxy_r_hops + 1)); [ \"$__muxy_r_hops\" -le 40 ] || return 1; "
            + "case \"$__muxy_r_link\" in "
            + "/*) __muxy_r_path=$__muxy_r_link ;; "
            + "*) __muxy_r_parent=${__muxy_r_path%/*}; [ -n \"$__muxy_r_parent\" ] || __muxy_r_parent=/; "
            + "__muxy_r_path=$__muxy_r_parent/$__muxy_r_link ;; "
            + "esac; "
            + "__muxy_r_path=\"$__muxy_r_path$__muxy_r_suffix\"; __muxy_r_suffix=; continue; "
            + "fi; "
            + "if [ -d \"$__muxy_r_path\" ]; then "
            + "__muxy_r_directory=$(cd \"$__muxy_r_path\" 2>/dev/null && { pwd -P; printf '\\001'; }) "
            + "|| return 1; "
            + "__muxy_r_directory=${__muxy_r_directory%?}; __muxy_r_directory=${__muxy_r_directory%?}; "
            + "printf '%s%s\\001' \"$__muxy_r_directory\" \"$__muxy_r_suffix\"; return; "
            + "fi; "
            + "__muxy_r_name=${__muxy_r_path##*/}; __muxy_r_parent=${__muxy_r_path%/*}; "
            + "[ -n \"$__muxy_r_parent\" ] || __muxy_r_parent=/; "
            + "__muxy_r_directory=$(cd \"$__muxy_r_parent\" 2>/dev/null && { pwd -P; printf '\\001'; }) "
            + "|| return 1; "
            + "__muxy_r_directory=${__muxy_r_directory%?}; __muxy_r_directory=${__muxy_r_directory%?}; "
            + "printf '%s/%s%s\\001' \"$__muxy_r_directory\" \"$__muxy_r_name\" \"$__muxy_r_suffix\"; "
            + "return; "
            + "done; "
            + "}; "
            + "__muxy_require_contained() { "
            + "__muxy_real=$(__muxy_resolve \"$1\") || exit \(containmentEscapeExitCode); "
            + "__muxy_real=${__muxy_real%?}; "
            + "case \"$__muxy_real\" in \"$__muxy_root\"|\"$__muxy_root\"/*) ;; "
            + "*) exit \(containmentEscapeExitCode) ;; esac; "
            + "}; "
            + "__muxy_require_contained \(quotedTarget); "
    }

    static func isValidEnvironmentKey(_ key: String) -> Bool {
        guard let first = key.first, first.isASCII, first.isLetter || first == "_" else { return false }
        return key.dropFirst().allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "_") }
    }

    static func environmentPrefix(_ environment: [String: String]?) -> String {
        guard let environment, !environment.isEmpty else { return "" }
        let assignments = SSHEnvironmentVariables.sanitize(environment)
            .sorted { $0.key < $1.key }
            .map { "export \($0.key)=\(ShellEscaper.escape($0.value))" }
        guard !assignments.isEmpty else { return "" }
        return assignments.joined(separator: "; ") + "; "
    }

    static func remoteCommand(
        executable: String,
        arguments: [String],
        workingDirectory: String?,
        environment: [String: String]? = nil
    ) -> String {
        let command = ([executable] + arguments)
            .map(quoteRemotePath)
            .joined(separator: " ")
        return environmentPrefix(environment)
            + changeDirectoryPrefix(workingDirectory)
            + command
    }

    static func remoteShellCommand(
        shell: String,
        workingDirectory: String?,
        environment: [String: String]? = nil
    ) -> String {
        environmentPrefix(environment)
            + changeDirectoryPrefix(workingDirectory)
            + "( \(shell) )"
    }
}
