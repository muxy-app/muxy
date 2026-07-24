import Darwin
import Foundation

public struct AgentHookFailureLogger {
    private let logFileURL: URL?
    private let maximumLogSize: Int

    public init(
        logFileURL: URL? = AgentHookPaths.defaultLogFileURL,
        maximumLogSize: Int = 1_048_576
    ) {
        self.logFileURL = logFileURL
        self.maximumLogSize = maximumLogSize
    }

    public func append(provider: String, event: String, error: any Error, timestamp: Int64) {
        guard let logFileURL else { return }

        do {
            try FileManager.default.createDirectory(
                at: logFileURL.deletingLastPathComponent(),
                withIntermediateDirectories: true,
                attributes: [.posixPermissions: 0o700]
            )
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o700],
                ofItemAtPath: logFileURL.deletingLastPathComponent().path
            )
            let record: [String: Any] = [
                "error": flattened(String(describing: error)),
                "event": flattened(event),
                "provider": flattened(provider),
                "ts": timestamp,
            ]
            var data = try JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])
            data.append(UInt8(ascii: "\n"))
            try append(data, to: logFileURL, maximumLogSize: maximumLogSize)
        } catch {}
    }

    private func append(_ data: Data, to url: URL, maximumLogSize: Int) throws {
        let descriptor = open(url.path, O_WRONLY | O_CREAT | O_APPEND, mode_t(0o600))
        guard descriptor >= 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        defer { close(descriptor) }
        guard fchmod(descriptor, mode_t(0o600)) == 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        guard flock(descriptor, LOCK_EX) == 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        defer { _ = flock(descriptor, LOCK_UN) }
        var fileStatus = stat()
        guard fstat(descriptor, &fileStatus) == 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }
        if fileStatus.st_size + off_t(data.count) > off_t(maximumLogSize) {
            guard ftruncate(descriptor, 0) == 0 else {
                throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            }
        }

        try data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress else { return }
            var offset = 0
            while offset < rawBuffer.count {
                let written = Darwin.write(
                    descriptor,
                    baseAddress.advanced(by: offset),
                    rawBuffer.count - offset
                )
                if written > 0 {
                    offset += written
                    continue
                }
                if written < 0, errno == EINTR {
                    continue
                }
                throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            }
        }
    }

    private func flattened(_ value: String) -> String {
        value.replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\r", with: " ")
    }
}
