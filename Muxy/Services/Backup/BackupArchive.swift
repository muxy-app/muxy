import Darwin
import Foundation
import UniformTypeIdentifiers

enum BackupArchiveError: LocalizedError {
    case archiveFailed(Int32)
    case archiveTimedOut
    case extractionFailed(Int32)
    case extractionTimedOut
    case manifestMissing
    case manifestUnsupported

    var errorDescription: String? {
        switch self {
        case let .archiveFailed(status):
            "Failed to create backup archive (ditto exited with status \(status))."
        case .archiveTimedOut:
            "Failed to create backup archive because the operation took too long."
        case let .extractionFailed(status):
            "Failed to read backup archive (ditto exited with status \(status))."
        case .extractionTimedOut:
            "Failed to read backup archive because the operation took too long."
        case .manifestMissing:
            "The selected file is not a valid Muxy backup."
        case .manifestUnsupported:
            "This backup was created by a newer version of Muxy and cannot be imported."
        }
    }
}

enum BackupArchive {
    static let fileExtension = "muxy"

    static let contentType = UTType(filenameExtension: fileExtension) ?? .data
    private static let processTimeout: TimeInterval = 600

    static let exportableFiles = [
        "settings.json",
        "projects.json",
        "project-groups.json",
        "remote-devices.json",
        "workspaces.json",
        "extension-shortcuts.json",
        "keybindings.json",
        "command-shortcuts.json",
        "editor-settings.json",
        "rich-input-drafts.json",
        "ghostty.conf",
    ]

    static let exportableDirectories = [
        "worktrees",
        "logos",
        "RichInputImages",
    ]

    static func zip(directory: URL, to archiveURL: URL) throws {
        switch runDitto(["-c", "-k", "--sequesterRsrc", directory.path, archiveURL.path]) {
        case .success:
            return
        case let .failed(status):
            throw BackupArchiveError.archiveFailed(status)
        case .timedOut:
            throw BackupArchiveError.archiveTimedOut
        }
    }

    static func unzip(archiveURL: URL, to directory: URL) throws {
        switch runDitto(["-x", "-k", archiveURL.path, directory.path]) {
        case .success:
            return
        case let .failed(status):
            throw BackupArchiveError.extractionFailed(status)
        case .timedOut:
            throw BackupArchiveError.extractionTimedOut
        }
    }

    private static func runDitto(_ arguments: [String]) -> DittoResult {
        let process = Process()
        let didExit = DispatchSemaphore(value: 0)
        process.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        process.arguments = arguments
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        process.terminationHandler = { _ in didExit.signal() }
        do {
            try process.run()
        } catch {
            return .failed(-1)
        }
        guard didExit.wait(timeout: .now() + processTimeout) == .success else {
            process.terminate()
            if didExit.wait(timeout: .now() + 2) == .timedOut {
                kill(process.processIdentifier, SIGKILL)
                process.waitUntilExit()
            }
            return .timedOut
        }
        guard process.terminationStatus == 0 else { return .failed(process.terminationStatus) }
        return .success
    }
}

private enum DittoResult {
    case success
    case failed(Int32)
    case timedOut
}
