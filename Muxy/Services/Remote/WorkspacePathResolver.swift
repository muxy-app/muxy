import Foundation

struct WorkspacePathResolution: Equatable, Sendable {
    let path: String
}

struct WorkspacePathCommandResult: Sendable {
    let status: Int32
    let stdoutData: Data
    let stderr: String
}

enum WorkspacePathResolverError: LocalizedError, Equatable {
    case commandFailed(String)
    case invalidOutput
    case invalidPath

    var errorDescription: String? {
        switch self {
        case let .commandFailed(message):
            message
        case .invalidOutput:
            "Remote path resolution returned invalid output."
        case .invalidPath:
            "Remote path resolution returned an invalid path."
        }
    }
}

protocol WorkspacePathResolving: Sendable {
    func resolve(
        paths: [String],
        relativeTo basePath: String,
        context: WorkspaceContext,
        timeout: TimeInterval
    ) async throws -> [WorkspacePathResolution]
}

struct WorkspacePathResolver: WorkspacePathResolving, Sendable {
    typealias RemoteRunner = @Sendable (
        _ destination: SSHDestination,
        _ command: String,
        _ timeout: TimeInterval
    ) async throws -> WorkspacePathCommandResult

    static let live = WorkspacePathResolver()
    private static let maxRemoteCommandBytes = 32 * 1024

    private let remoteRunner: RemoteRunner

    init(remoteRunner: @escaping RemoteRunner = { destination, command, timeout in
        let result = try await SSHCommandRunner.run(
            destination: destination,
            remoteCommand: command,
            timeout: timeout
        )
        return WorkspacePathCommandResult(
            status: result.status,
            stdoutData: result.stdoutData,
            stderr: result.stderr
        )
    }) {
        self.remoteRunner = remoteRunner
    }

    func resolve(
        paths: [String],
        relativeTo basePath: String,
        context: WorkspaceContext,
        timeout: TimeInterval
    ) async throws -> [WorkspacePathResolution] {
        guard !paths.isEmpty else { return [] }
        guard case let .ssh(destination) = context else {
            return paths.map {
                WorkspacePathResolution(path: Self.localPath($0, relativeTo: basePath))
            }
        }

        let deadline = OperationDeadline(timeout: timeout)
        var resolutions: [WorkspacePathResolution] = []
        for batch in Self.remotePathBatches(paths: paths, basePath: basePath) {
            let result = try await remoteRunner(
                destination,
                Self.remoteCommand(paths: batch, basePath: basePath),
                deadline.remaining()
            )
            guard result.status == 0 else {
                throw WorkspacePathResolverError.commandFailed(
                    result.stderr.isEmpty ? "Failed to resolve remote paths." : result.stderr
                )
            }
            resolutions += try Self.parseRemoteResolutions(result.stdoutData, expectedCount: batch.count)
        }
        return resolutions
    }

    private static func localPath(_ path: String, relativeTo basePath: String) -> String {
        let expandedPath = NSString(string: path).expandingTildeInPath
        let absolutePath: String
        if expandedPath.hasPrefix("/") {
            absolutePath = expandedPath
        } else {
            let expandedBase = NSString(string: basePath).expandingTildeInPath
            absolutePath = URL(fileURLWithPath: expandedBase, isDirectory: true)
                .appendingPathComponent(expandedPath)
                .path
        }

        let standardized = URL(fileURLWithPath: absolutePath).standardizedFileURL
        let resolved = standardized.resolvingSymlinksInPath()
        guard resolved.path == standardized.path else { return resolved.path }
        let parent = standardized.deletingLastPathComponent().resolvingSymlinksInPath()
        return parent.appendingPathComponent(standardized.lastPathComponent).path
    }

    private static func remoteCommand(paths: [String], basePath: String) -> String {
        let arguments = ["/bin/sh", "-c", remoteResolutionScript, "muxy-path-resolver", basePath] + paths
        return arguments.map(ShellEscaper.escape).joined(separator: " ")
    }

    private static func remotePathBatches(paths: [String], basePath: String) -> [[String]] {
        let baseBytes = remoteCommand(paths: [], basePath: basePath).utf8.count
        var batches: [[String]] = []
        var batch: [String] = []
        var batchBytes = baseBytes
        for path in paths {
            let pathBytes = 1 + ShellEscaper.escape(path).utf8.count
            if !batch.isEmpty, batchBytes + pathBytes > maxRemoteCommandBytes {
                batches.append(batch)
                batch = []
                batchBytes = baseBytes
            }
            batch.append(path)
            batchBytes += pathBytes
        }
        if !batch.isEmpty {
            batches.append(batch)
        }
        return batches
    }

    private static func parseRemoteResolutions(
        _ data: Data,
        expectedCount: Int
    ) throws -> [WorkspacePathResolution] {
        let fields = data.split(separator: 0, omittingEmptySubsequences: false)
        guard fields.count == expectedCount + 1 else {
            throw WorkspacePathResolverError.invalidOutput
        }
        return try fields.dropLast().map {
            guard let path = String(bytes: $0, encoding: .utf8) else {
                throw WorkspacePathResolverError.invalidPath
            }
            return WorkspacePathResolution(path: ProjectPickerPathService.standardizedRemotePath(path))
        }
    }

    private static let remoteResolutionScript = """
    __muxy_base=$1
    shift
    case "$__muxy_base" in
      "~") [ -n "$HOME" ] && __muxy_base=$HOME ;;
      "~/"*) [ -n "$HOME" ] && __muxy_base=$HOME/${__muxy_base#??} ;;
      /*) ;;
      *) __muxy_base=$PWD/$__muxy_base ;;
    esac
    for __muxy_input in "$@"; do
      case "$__muxy_input" in
        "~") [ -n "$HOME" ] || exit 10; __muxy_candidate=$HOME ;;
        "~/"*) [ -n "$HOME" ] || exit 10; __muxy_candidate=$HOME/${__muxy_input#??} ;;
        /*) __muxy_candidate=$__muxy_input ;;
        *) case "$__muxy_base" in "~"|"~/"*) exit 10 ;; esac; __muxy_candidate=$__muxy_base/$__muxy_input ;;
      esac
      __muxy_probe=$__muxy_candidate
      __muxy_suffix=
      while [ ! -d "$__muxy_probe" ] && [ "$__muxy_probe" != / ]; do
        __muxy_name=${__muxy_probe##*/}
        __muxy_parent=${__muxy_probe%/*}
        [ -n "$__muxy_parent" ] || __muxy_parent=/
        [ "$__muxy_parent" != "$__muxy_probe" ] || break
        __muxy_suffix=/$__muxy_name$__muxy_suffix
        __muxy_probe=$__muxy_parent
      done
      if [ -d "$__muxy_probe" ]; then
        __muxy_physical=$(cd "$__muxy_probe" 2>/dev/null && pwd -P)
        if [ -n "$__muxy_physical" ]; then
          __muxy_resolved=$__muxy_physical$__muxy_suffix
        else
          __muxy_resolved=$__muxy_candidate
        fi
      else
        __muxy_resolved=$__muxy_candidate
      fi
      printf '%s\\0' "$__muxy_resolved"
    done
    """
}
