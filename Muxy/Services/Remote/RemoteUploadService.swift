import Foundation
import OSLog

private let logger = Logger(subsystem: "app.muxy", category: "RemoteUploadService")

enum RemoteUploadError: LocalizedError {
    case invalidIdentifier
    case uploadFailed(String)
    case invalidRemotePath

    var errorDescription: String? {
        switch self {
        case .invalidIdentifier:
            "The upload identifier is invalid."
        case let .uploadFailed(detail):
            detail.isEmpty ? "The upload failed." : detail
        case .invalidRemotePath:
            "The remote device returned an invalid path."
        }
    }
}

enum RemoteUploadService {
    static let maximumByteCount = 100 * 1024 * 1024

    private static let pathMarker = "MUXY_UPLOAD_PATH="
    private static let minimumThroughputBytesPerSecond = 256 * 1024
    private static let maximumExtensionLength = 16

    static func upload(
        data: Data,
        fileExtension: String?,
        destination: SSHDestination,
        sessionID: String,
        uploadID: String
    ) async throws -> String {
        guard isValidIdentifier(sessionID), isValidIdentifier(uploadID) else {
            throw RemoteUploadError.invalidIdentifier
        }
        let result = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: uploadCommand(
                sessionID: sessionID,
                uploadID: uploadID,
                fileExtension: fileExtension
            ),
            timeout: timeout(forByteCount: data.count),
            input: data
        )
        return try uploadedPath(
            from: result,
            sessionID: sessionID,
            uploadID: uploadID,
            fileExtension: fileExtension
        )
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
                logger.error("Remote upload cleanup failed with status \(result.status)")
                return
            }
        } catch {
            logger.error("Remote upload cleanup failed: \(error.localizedDescription)")
        }
    }

    nonisolated static func uploadCommand(
        sessionID: String,
        uploadID: String,
        fileExtension: String?
    ) -> String {
        let directory = directoryName(sessionID: sessionID)
        let file = fileName(uploadID: uploadID, fileExtension: fileExtension)
        return "umask 077; "
            + "__muxy_base=${TMPDIR:-/tmp}; "
            + "cd \"$__muxy_base\" || exit 1; "
            + "__muxy_dir=\(directory); "
            + "if [ -e \"$__muxy_dir\" ]; "
            + "then [ -d \"$__muxy_dir\" ] && [ ! -L \"$__muxy_dir\" ] || exit 1; "
            + "else mkdir -m 700 \"$__muxy_dir\" || exit 1; fi; "
            + "chmod 700 \"$__muxy_dir\" || exit 1; "
            + "__muxy_path=\"$(pwd -P)/$__muxy_dir/\(file)\"; "
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
        uploadID: String,
        fileExtension: String?
    ) throws -> String {
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            throw RemoteUploadError.uploadFailed(detail)
        }
        let path = result.stdout
            .split(whereSeparator: \.isNewline)
            .reversed()
            .first { $0.hasPrefix(pathMarker) }
            .map { String($0.dropFirst(pathMarker.count)) }
        guard let path, isManagedPath(
            path,
            sessionID: sessionID,
            uploadID: uploadID,
            fileExtension: fileExtension
        )
        else {
            throw RemoteUploadError.invalidRemotePath
        }
        return path
    }

    nonisolated static func cleanupCommand(sessionID: String) -> String? {
        guard isValidIdentifier(sessionID) else { return nil }
        let directory = directoryName(sessionID: sessionID)
        return "__muxy_base=${TMPDIR:-/tmp}; "
            + "cd \"$__muxy_base\" || exit 1; "
            + "__muxy_dir=\(directory); "
            + "if [ ! -e \"$__muxy_dir\" ]; then exit 0; fi; "
            + "[ -d \"$__muxy_dir\" ] && [ ! -L \"$__muxy_dir\" ] || exit 1; "
            + "rm -f \"$__muxy_dir\"/*; "
            + "__muxy_status=$?; "
            + "rmdir \"$__muxy_dir\" 2>/dev/null || true; "
            + "exit $__muxy_status"
    }

    nonisolated static func isManagedPath(
        _ path: String,
        sessionID: String,
        uploadID: String,
        fileExtension: String?
    ) -> Bool {
        guard isValidIdentifier(sessionID), isValidIdentifier(uploadID) else { return false }
        let containsControlCharacter = path.unicodeScalars.contains {
            CharacterSet.controlCharacters.contains($0)
        }
        guard path.hasPrefix("/"), !containsControlCharacter else { return false }
        let fileURL = URL(fileURLWithPath: path)
        guard fileURL.lastPathComponent == fileName(uploadID: uploadID, fileExtension: fileExtension) else {
            return false
        }
        return fileURL.deletingLastPathComponent().lastPathComponent == directoryName(sessionID: sessionID)
    }

    nonisolated static func isValidIdentifier(_ identifier: String) -> Bool {
        guard (8 ... 64).contains(identifier.count) else { return false }
        return identifier.allSatisfy {
            $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-")
        }
    }

    nonisolated static func sanitizedExtension(for url: URL) -> String? {
        let lowercased = url.pathExtension.lowercased()
        guard !lowercased.isEmpty, lowercased.count <= maximumExtensionLength else { return nil }
        let isSafe = lowercased.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber) }
        return isSafe ? lowercased : nil
    }

    nonisolated static func timeout(forByteCount byteCount: Int) -> TimeInterval {
        max(SSHCommandRunner.defaultTimeout, Double(byteCount) / Double(minimumThroughputBytesPerSecond))
    }

    nonisolated private static func directoryName(sessionID: String) -> String {
        "muxy-uploads.\(sessionID)"
    }

    nonisolated private static func fileName(uploadID: String, fileExtension: String?) -> String {
        guard let fileExtension, !fileExtension.isEmpty else { return uploadID }
        return "\(uploadID).\(fileExtension)"
    }
}
