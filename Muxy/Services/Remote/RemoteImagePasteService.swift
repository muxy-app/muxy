import Foundation
import OSLog

private let logger = Logger(subsystem: "app.muxy", category: "RemoteImagePasteService")

enum RemoteImagePasteError: LocalizedError {
    case emptyImage
    case invalidIdentifier
    case uploadFailed(String)
    case invalidRemotePath

    var errorDescription: String? {
        switch self {
        case .emptyImage:
            "The clipboard image is empty."
        case .invalidIdentifier:
            "The image upload identifier is invalid."
        case let .uploadFailed(detail):
            detail.isEmpty ? "The image upload failed." : detail
        case .invalidRemotePath:
            "The remote device returned an invalid image path."
        }
    }
}

enum RemoteImagePasteService {
    private static let pathMarker = "MUXY_IMAGE_PATH="

    static func upload(
        pngData: Data,
        destination: SSHDestination,
        sessionID: String,
        imageID: String
    ) async throws -> String {
        guard !pngData.isEmpty else { throw RemoteImagePasteError.emptyImage }
        guard isValidIdentifier(sessionID), isValidIdentifier(imageID) else {
            throw RemoteImagePasteError.invalidIdentifier
        }
        let result = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: uploadCommand(sessionID: sessionID, imageID: imageID),
            input: pngData
        )
        return try uploadedPath(from: result, sessionID: sessionID, imageID: imageID)
    }

    static func remove(sessionID: String, destination: SSHDestination) async {
        guard let command = cleanupCommand(sessionID: sessionID) else { return }
        do {
            let result = try await SSHCommandRunner.run(
                destination: destination,
                remoteCommand: command,
                timeout: 15
            )
            guard result.status == 0 else {
                logger.error("Remote image cleanup failed with status \(result.status)")
                return
            }
        } catch {
            logger.error("Remote image cleanup failed: \(error.localizedDescription)")
        }
    }

    nonisolated static func uploadCommand(sessionID: String, imageID: String) -> String {
        let directoryName = "muxy-images.\(sessionID)"
        let fileName = "\(imageID).png"
        return "umask 077; "
            + "__muxy_base=${TMPDIR:-/tmp}; "
            + "cd \"$__muxy_base\" || exit 1; "
            + "__muxy_dir=\(directoryName); "
            + "if [ -e \"$__muxy_dir\" ]; "
            + "then [ -d \"$__muxy_dir\" ] && [ ! -L \"$__muxy_dir\" ] || exit 1; "
            + "else mkdir -m 700 \"$__muxy_dir\" || exit 1; fi; "
            + "chmod 700 \"$__muxy_dir\" || exit 1; "
            + "__muxy_path=\"$(pwd -P)/$__muxy_dir/\(fileName)\"; "
            + "__muxy_partial=\"$__muxy_path.part\"; "
            + "trap 'rm -f \"$__muxy_partial\"' EXIT HUP INT TERM; "
            + "if cat > \"$__muxy_partial\" && chmod 600 \"$__muxy_partial\" "
            + "&& mv -f \"$__muxy_partial\" \"$__muxy_path\"; "
            + "then trap - EXIT HUP INT TERM; printf '\\n\(pathMarker)%s\\n' \"$__muxy_path\"; "
            + "else exit 1; fi"
    }

    nonisolated static func uploadedPath(
        from result: GitProcessResult,
        sessionID: String,
        imageID: String
    ) throws -> String {
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            throw RemoteImagePasteError.uploadFailed(detail)
        }
        let path = result.stdout
            .split(whereSeparator: \.isNewline)
            .reversed()
            .first { $0.hasPrefix(pathMarker) }
            .map { String($0.dropFirst(pathMarker.count)) }
        guard let path, isManagedPath(path, sessionID: sessionID, imageID: imageID) else {
            throw RemoteImagePasteError.invalidRemotePath
        }
        return path
    }

    nonisolated static func cleanupCommand(sessionID: String) -> String? {
        guard isValidIdentifier(sessionID) else { return nil }
        let directoryName = "muxy-images.\(sessionID)"
        return "__muxy_base=${TMPDIR:-/tmp}; "
            + "cd \"$__muxy_base\" || exit 1; "
            + "__muxy_dir=\(directoryName); "
            + "if [ ! -e \"$__muxy_dir\" ]; then exit 0; fi; "
            + "[ -d \"$__muxy_dir\" ] && [ ! -L \"$__muxy_dir\" ] || exit 1; "
            + "rm -f \"$__muxy_dir\"/*.png \"$__muxy_dir\"/*.part; "
            + "__muxy_status=$?; "
            + "rmdir \"$__muxy_dir\" 2>/dev/null || true; "
            + "exit $__muxy_status"
    }

    nonisolated static func isManagedPath(
        _ path: String,
        sessionID: String,
        imageID: String
    ) -> Bool {
        guard isValidIdentifier(sessionID), isValidIdentifier(imageID) else { return false }
        let containsControlCharacter = path.unicodeScalars.contains {
            CharacterSet.controlCharacters.contains($0)
        }
        guard path.hasPrefix("/"), !containsControlCharacter else { return false }
        let fileURL = URL(fileURLWithPath: path)
        guard fileURL.lastPathComponent == "\(imageID).png" else { return false }
        return fileURL.deletingLastPathComponent().lastPathComponent == "muxy-images.\(sessionID)"
    }

    nonisolated static func isValidIdentifier(_ identifier: String) -> Bool {
        guard (8 ... 64).contains(identifier.count) else { return false }
        return identifier.allSatisfy {
            $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-")
        }
    }
}
