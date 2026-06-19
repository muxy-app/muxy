import Foundation
import UniformTypeIdentifiers

enum BackupArchiveError: LocalizedError {
    case archiveFailed(Int32)
    case extractionFailed(Int32)
    case manifestMissing
    case manifestUnsupported

    var errorDescription: String? {
        switch self {
        case let .archiveFailed(status):
            "Failed to create backup archive (ditto exited with status \(status))."
        case let .extractionFailed(status):
            "Failed to read backup archive (ditto exited with status \(status))."
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
        let status = runDitto(["-c", "-k", "--sequesterRsrc", directory.path, archiveURL.path])
        guard status == 0 else { throw BackupArchiveError.archiveFailed(status) }
    }

    static func unzip(archiveURL: URL, to directory: URL) throws {
        let status = runDitto(["-x", "-k", archiveURL.path, directory.path])
        guard status == 0 else { throw BackupArchiveError.extractionFailed(status) }
    }

    private static func runDitto(_ arguments: [String]) -> Int32 {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/ditto")
        process.arguments = arguments
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice
        do {
            try process.run()
        } catch {
            return -1
        }
        process.waitUntilExit()
        return process.terminationStatus
    }
}
