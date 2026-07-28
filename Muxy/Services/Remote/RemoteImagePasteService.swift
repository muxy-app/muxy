import Foundation
import OSLog

private let logger = Logger(subsystem: "app.muxy", category: "RemoteImagePasteService")

enum RemoteImagePasteError: LocalizedError {
    case emptyImage
    case uploadFailed(String)
    case invalidRemotePath

    var errorDescription: String? {
        switch self {
        case .emptyImage:
            "The clipboard image is empty."
        case let .uploadFailed(detail):
            detail.isEmpty ? "The image upload failed." : detail
        case .invalidRemotePath:
            "The remote device returned an invalid image path."
        }
    }
}

enum RemoteImagePasteService {
    private static let pathMarker = "MUXY_IMAGE_PATH="

    static func upload(pngData: Data, destination: SSHDestination) async throws -> String {
        guard !pngData.isEmpty else { throw RemoteImagePasteError.emptyImage }
        let result = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: uploadCommand,
            input: pngData
        )
        return try uploadedPath(from: result)
    }

    static func remove(paths: Set<String>, destination: SSHDestination) async {
        guard let command = cleanupCommand(paths: paths) else { return }
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

    nonisolated static var uploadCommand: String {
        "umask 077; "
            + "__muxy_base=${TMPDIR:-/tmp}; "
            + "cd \"$__muxy_base\" || exit 1; "
            + "__muxy_dir=$(mktemp -d muxy-image.XXXXXXXX) || exit 1; "
            + "__muxy_path=\"$(pwd -P)/$__muxy_dir/image.png\"; "
            + "if cat > \"$__muxy_path\" && chmod 600 \"$__muxy_path\"; "
            + "then printf '\\n\(pathMarker)%s\\n' \"$__muxy_path\"; "
            + "else rm -f \"$__muxy_path\"; rmdir \"$__muxy_dir\" 2>/dev/null; exit 1; fi"
    }

    nonisolated static func uploadedPath(from result: GitProcessResult) throws -> String {
        guard result.status == 0 else {
            let detail = result.stderr.trimmingCharacters(in: .whitespacesAndNewlines)
            throw RemoteImagePasteError.uploadFailed(detail)
        }
        let path = result.stdout
            .split(whereSeparator: \.isNewline)
            .reversed()
            .first { $0.hasPrefix(pathMarker) }
            .map { String($0.dropFirst(pathMarker.count)) }
        guard let path, isManagedPath(path) else {
            throw RemoteImagePasteError.invalidRemotePath
        }
        return path
    }

    nonisolated static func cleanupCommand(paths: Set<String>) -> String? {
        let managedPaths = paths.filter(isManagedPath).sorted()
        guard !managedPaths.isEmpty else { return nil }
        let files = managedPaths.map(ShellEscaper.escape).joined(separator: " ")
        let directories = Set(managedPaths.map { ($0 as NSString).deletingLastPathComponent })
            .sorted()
            .map(ShellEscaper.escape)
            .joined(separator: " ")
        return "rm -f \(files); rmdir \(directories) 2>/dev/null || true"
    }

    nonisolated static func isManagedPath(_ path: String) -> Bool {
        let containsControlCharacter = path.unicodeScalars.contains {
            CharacterSet.controlCharacters.contains($0)
        }
        guard path.hasPrefix("/"), !containsControlCharacter else {
            return false
        }
        let fileURL = URL(fileURLWithPath: path)
        guard fileURL.lastPathComponent == "image.png" else { return false }
        let directory = fileURL.deletingLastPathComponent().lastPathComponent
        guard directory.hasPrefix("muxy-image.") else { return false }
        let suffix = directory.dropFirst("muxy-image.".count)
        return suffix.count >= 6 && suffix.allSatisfy {
            $0.isASCII && ($0.isLetter || $0.isNumber)
        }
    }
}
