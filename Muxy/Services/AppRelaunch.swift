import AppKit
import Foundation

enum AppRelaunch {
    @MainActor private(set) static var isRelaunching = false

    @MainActor
    static func prepareForRelaunch() {
        isRelaunching = true
    }

    @MainActor
    static func relaunch() {
        prepareForRelaunch()

        let bundleURL = Bundle.main.bundleURL
        guard bundleURL.pathExtension == "app" else {
            NSApp.terminate(nil)
            return
        }

        let pid = ProcessInfo.processInfo.processIdentifier
        let quotedPath = ShellEscaper.escape(bundleURL.path)
        let waitAndReopen = "while /bin/kill -0 \(pid) 2>/dev/null; do /bin/sleep 0.1; done; /usr/bin/open \(quotedPath)"

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/sh")
        process.arguments = ["-c", waitAndReopen]
        try? process.run()

        NSApp.terminate(nil)
    }

    @MainActor
    static func resetForTesting() {
        isRelaunching = false
    }
}
