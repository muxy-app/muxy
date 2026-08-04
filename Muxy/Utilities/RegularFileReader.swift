import Darwin
import Foundation

enum RegularFileReadError: LocalizedError, Equatable {
    case unsupportedFileType
    case tooLarge(maximumByteCount: Int)

    var errorDescription: String? {
        switch self {
        case .unsupportedFileType:
            "Only regular files can be attached."
        case let .tooLarge(maximumByteCount):
            "The file exceeds the \(maximumByteCount / (1024 * 1024)) MB limit."
        }
    }
}

enum RegularFileReader {
    static func data(contentsOf url: URL, maximumByteCount: Int) throws -> Data {
        let values = try url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
        try validateMetadata(
            isRegularFile: values.isRegularFile,
            fileSize: values.fileSize,
            maximumByteCount: maximumByteCount
        )
        let (handle, openedFileSize) = try openRegularFile(at: url, maximumByteCount: maximumByteCount)
        defer { try? handle.close() }

        let cappedByteCount = maximumByteCount + 1
        var data = Data()
        data.reserveCapacity(min(openedFileSize, maximumByteCount))
        while data.count < cappedByteCount {
            let readCount = min(64 * 1024, cappedByteCount - data.count)
            let chunk = try handle.read(upToCount: readCount) ?? Data()
            guard !chunk.isEmpty else { break }
            data.append(chunk)
        }
        return try validateByteCount(data, maximumByteCount: maximumByteCount)
    }

    static func validateMetadata(
        isRegularFile: Bool?,
        fileSize: Int?,
        maximumByteCount: Int
    ) throws {
        guard isRegularFile == true else {
            throw RegularFileReadError.unsupportedFileType
        }
        guard let fileSize else { return }
        guard fileSize >= 0 else {
            throw RegularFileReadError.unsupportedFileType
        }
        guard fileSize <= maximumByteCount else {
            throw RegularFileReadError.tooLarge(maximumByteCount: maximumByteCount)
        }
    }

    static func validateByteCount(_ data: Data, maximumByteCount: Int) throws -> Data {
        guard data.count <= maximumByteCount else {
            throw RegularFileReadError.tooLarge(maximumByteCount: maximumByteCount)
        }
        return data
    }

    private static func openRegularFile(
        at url: URL,
        maximumByteCount: Int
    ) throws -> (FileHandle, Int) {
        let descriptor = url.withUnsafeFileSystemRepresentation { path in
            guard let path else { return Int32(-1) }
            return Darwin.open(path, O_RDONLY | O_NONBLOCK | O_CLOEXEC)
        }
        guard descriptor >= 0 else {
            throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
        }

        var fileStatus = stat()
        guard fstat(descriptor, &fileStatus) == 0 else {
            let error = POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
            Darwin.close(descriptor)
            throw error
        }
        guard fileStatus.st_mode & S_IFMT == S_IFREG else {
            Darwin.close(descriptor)
            throw RegularFileReadError.unsupportedFileType
        }
        guard let fileSize = Int(exactly: fileStatus.st_size), fileSize >= 0 else {
            Darwin.close(descriptor)
            throw RegularFileReadError.unsupportedFileType
        }
        guard fileSize <= maximumByteCount else {
            Darwin.close(descriptor)
            throw RegularFileReadError.tooLarge(maximumByteCount: maximumByteCount)
        }
        return (FileHandle(fileDescriptor: descriptor, closeOnDealloc: true), fileSize)
    }
}
