import AppKit
import Foundation

enum AppRelaunch {
    @MainActor
    static func relaunch() {
        let process = Process()
        let bundleURL = Bundle.main.bundleURL
        if bundleURL.pathExtension == "app" {
            process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
            process.arguments = ["-n", bundleURL.path]
        } else if let executableURL = Bundle.main.executableURL {
            process.executableURL = executableURL
        } else {
            NSApp.terminate(nil)
            return
        }
        try? process.run()
        NSApp.terminate(nil)
    }
}
