import Foundation

enum CLIWrapperScript {
    static let bundleIdentifier = "com.muxy.app"
    static let bundledScriptRelativePath = "Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    static let currentFormatVersion = 1

    static func requiresMigration(
        _ contents: String,
        targetVersion: Int = currentFormatVersion
    ) -> Bool {
        guard contents.hasPrefix("#!/bin/bash\n# Muxy CLI wrapper") else { return false }
        if contents.contains(bundledScriptRelativePath) {
            guard let installedVersion = formatVersion(in: contents) else {
                return targetVersion > 1
            }
            return installedVersion < targetVersion
        }
        return contents.contains("MUXY_SOCKET_PATH") || contents.contains("muxy://open")
    }

    static func contents(installedAppPath: String) -> String {
        let escapedAppPath = ShellEscaper.escape(installedAppPath)
        let escapedRelativePath = ShellEscaper.escape(bundledScriptRelativePath)
        let escapedBundleID = ShellEscaper.escape(bundleIdentifier)
        return """
        #!/bin/bash
        # Muxy CLI wrapper version \(currentFormatVersion)
        # Resolves the bundled muxy-cli at runtime so it never goes stale across
        # app updates and survives the app being moved.
        REL=\(escapedRelativePath)

        resolve_script() {
            local app="$1"
            [ -n "$app" ] && [ -x "$app/$REL" ] && printf '%s' "$app/$REL"
        }

        for candidate in \\
            "${MUXY_APP_PATH:-}" \\
            \(escapedAppPath) \\
            "/Applications/Muxy.app" \\
            "$HOME/Applications/Muxy.app"; do
            SCRIPT="$(resolve_script "$candidate")"
            [ -n "$SCRIPT" ] && exec "$SCRIPT" "$@"
        done

        APP="$(mdfind "kMDItemCFBundleIdentifier == \(escapedBundleID)" 2>/dev/null | head -n 1)"
        SCRIPT="$(resolve_script "$APP")"
        [ -n "$SCRIPT" ] && exec "$SCRIPT" "$@"

        echo "Error: Muxy.app not found. Reinstall the CLI from Muxy → Install CLI." >&2
        exit 1
        """
    }

    private static func formatVersion(in contents: String) -> Int? {
        let prefix = "# Muxy CLI wrapper version "
        guard let line = contents.split(separator: "\n").first(where: { $0.hasPrefix(prefix) }) else {
            return nil
        }
        return Int(line.dropFirst(prefix.count))
    }
}
