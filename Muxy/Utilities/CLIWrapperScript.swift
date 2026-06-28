import Foundation

enum CLIWrapperScript {
    static let bundleIdentifier = "com.muxy.app"
    static let bundledScriptRelativePath = "Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"

    static func contents(installedAppPath: String) -> String {
        let escapedAppPath = ShellEscaper.escape(installedAppPath)
        let escapedRelativePath = ShellEscaper.escape(bundledScriptRelativePath)
        let escapedBundleID = ShellEscaper.escape(bundleIdentifier)
        return """
        #!/bin/bash
        # Muxy CLI wrapper. Resolves the bundled muxy-cli at runtime so it never
        # goes stale across app updates and survives the app being moved.
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
}
