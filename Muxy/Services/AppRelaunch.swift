import AppKit
import Foundation

enum AppRelaunchError: LocalizedError {
    case launchFailed(String)

    var errorDescription: String? {
        switch self {
        case let .launchFailed(message):
            "Failed to restart Muxy: \(message)"
        }
    }
}

struct AppRelaunchRequest: Equatable {
    let executableURL: URL
    let arguments: [String]
}

enum AppRelaunch {
    @MainActor private(set) static var isRelaunching = false
    private static let waitAndOpenScript = "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.1; done; /usr/bin/open \"$2\""
    private static let waitAndRunScript = "while /bin/kill -0 \"$1\" 2>/dev/null; do /bin/sleep 0.1; done; \"$2\""

    @MainActor
    static func prepareForRelaunch() {
        isRelaunching = true
    }

    @MainActor
    static func relaunch() throws {
        prepareForRelaunch()

        let bundleURL = Bundle.main.bundleURL
        let request = launchRequest(
            bundleURL: bundleURL,
            executableURL: Bundle.main.executableURL,
            processID: ProcessInfo.processInfo.processIdentifier
        )
        do {
            try launch(request)
        } catch {
            throw error
        }

        dismissAttachedSheets()
        DispatchQueue.main.async {
            NSApp.terminate(nil)
        }
    }

    @MainActor
    static func dismissAttachedSheets(in windows: [NSWindow]) {
        for window in windows {
            guard let sheet = window.attachedSheet else { continue }
            window.endSheet(sheet)
        }
    }

    @MainActor
    private static func dismissAttachedSheets() {
        dismissAttachedSheets(in: NSApp.windows)
    }

    nonisolated static func launchRequest(bundleURL: URL, executableURL: URL?, processID: Int32) -> AppRelaunchRequest {
        if bundleURL.pathExtension == "app" {
            return request(script: waitAndOpenScript, targetPath: bundleURL.path, processID: processID)
        }
        return request(script: waitAndRunScript, targetPath: executableURL?.path ?? bundleURL.path, processID: processID)
    }

    private static func launch(_ request: AppRelaunchRequest) throws {
        let process = Process()
        process.executableURL = request.executableURL
        process.arguments = request.arguments
        process.standardInput = FileHandle.nullDevice
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
        } catch {
            throw AppRelaunchError.launchFailed(error.localizedDescription)
        }
    }

    private static func request(script: String, targetPath: String, processID: Int32) -> AppRelaunchRequest {
        AppRelaunchRequest(
            executableURL: URL(fileURLWithPath: "/usr/bin/nohup"),
            arguments: ["/bin/sh", "-c", script, "muxy-relaunch", "\(processID)", targetPath]
        )
    }

    @MainActor
    static func resetForTesting() {
        isRelaunching = false
    }
}
