import AppKit
import Darwin
import ImageIO
import UniformTypeIdentifiers

enum ImagePasteDataError: LocalizedError {
    case missingImage
    case unsupportedFileType
    case encodedImageTooLarge
    case pixelCountTooLarge
    case invalidImage
    case pngEncodingFailed

    var errorDescription: String? {
        switch self {
        case .missingImage:
            "Could not read the image."
        case .unsupportedFileType:
            "Only regular image files can be attached."
        case .encodedImageTooLarge:
            "The image exceeds the 25 MB limit."
        case .pixelCountTooLarge:
            "The image exceeds the 64 megapixel limit."
        case .invalidImage:
            "The image data is invalid."
        case .pngEncodingFailed:
            "Could not convert the image to PNG."
        }
    }
}

@MainActor
enum ImagePasteData {
    nonisolated static let maximumEncodedByteCount = 25 * 1024 * 1024
    nonisolated static let maximumPixelCount = 64_000_000

    private static let readablePasteboardTypes: [NSPasteboard.PasteboardType] = {
        let imageSourceTypes = CGImageSourceCopyTypeIdentifiers() as? [String] ?? []
        var identifiers = [UTType.png.identifier]
        identifiers.append(contentsOf: imageSourceTypes)
        var seenIdentifiers = Set<String>()
        return identifiers.compactMap { identifier in
            guard seenIdentifiers.insert(identifier).inserted else { return nil }
            return NSPasteboard.PasteboardType(identifier)
        }
    }()

    static func hasImage(in pasteboard: NSPasteboard = .general) -> Bool {
        guard pasteboard.string(forType: .string) == nil else { return false }
        return pasteboard.availableType(from: readablePasteboardTypes) != nil
    }

    static func sourceData(from pasteboard: NSPasteboard = .general) throws -> Data {
        guard hasImage(in: pasteboard),
              let type = pasteboard.availableType(from: readablePasteboardTypes),
              let data = pasteboard.data(forType: type)
        else {
            throw ImagePasteDataError.missingImage
        }
        return try validateEncodedSize(data)
    }

    static func pngData(from sourceData: Data) async throws -> Data {
        try await Task.detached(priority: .userInitiated) {
            try normalizedPNGData(from: sourceData)
        }.value
    }

    static func pngData(contentsOf url: URL) async throws -> Data {
        try await Task.detached(priority: .userInitiated) {
            let data = try encodedImageData(contentsOf: url)
            return try normalizedPNGData(from: data)
        }.value
    }

    nonisolated static func encodedImageData(contentsOf url: URL) throws -> Data {
        let values = try url.resourceValues(forKeys: [.isRegularFileKey, .fileSizeKey])
        try validateFileMetadata(
            isRegularFile: values.isRegularFile,
            fileSize: values.fileSize
        )
        let (handle, openedFileSize) = try openRegularFile(at: url)
        defer { try? handle.close() }

        let cappedByteCount = maximumEncodedByteCount + 1
        var data = Data()
        data.reserveCapacity(min(openedFileSize, maximumEncodedByteCount))
        while data.count < cappedByteCount {
            let readCount = min(64 * 1024, cappedByteCount - data.count)
            let chunk = try handle.read(upToCount: readCount) ?? Data()
            guard !chunk.isEmpty else { break }
            data.append(chunk)
        }
        return try validateEncodedSize(data)
    }

    nonisolated static func validateFileMetadata(
        isRegularFile: Bool?,
        fileSize: Int?
    ) throws {
        guard isRegularFile == true else {
            throw ImagePasteDataError.unsupportedFileType
        }
        guard let fileSize else { return }
        guard fileSize >= 0 else {
            throw ImagePasteDataError.missingImage
        }
        guard fileSize <= maximumEncodedByteCount else {
            throw ImagePasteDataError.encodedImageTooLarge
        }
    }

    nonisolated private static func openRegularFile(at url: URL) throws -> (FileHandle, Int) {
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
            throw ImagePasteDataError.unsupportedFileType
        }
        guard let fileSize = Int(exactly: fileStatus.st_size), fileSize >= 0 else {
            Darwin.close(descriptor)
            throw ImagePasteDataError.missingImage
        }
        guard fileSize <= maximumEncodedByteCount else {
            Darwin.close(descriptor)
            throw ImagePasteDataError.encodedImageTooLarge
        }
        return (FileHandle(fileDescriptor: descriptor, closeOnDealloc: true), fileSize)
    }

    nonisolated static func normalizedPNGData(from data: Data) throws -> Data {
        let validatedData = try validateEncodedSize(data)
        guard let source = CGImageSourceCreateWithData(validatedData as CFData, nil) else {
            throw ImagePasteDataError.invalidImage
        }
        try validatePixelCount(source: source)
        if CGImageSourceGetType(source) as String? == UTType.png.identifier {
            return validatedData
        }
        guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw ImagePasteDataError.invalidImage
        }
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(
            output,
            UTType.png.identifier as CFString,
            1,
            nil
        )
        else {
            throw ImagePasteDataError.pngEncodingFailed
        }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else {
            throw ImagePasteDataError.pngEncodingFailed
        }
        return try validateEncodedSize(output as Data)
    }

    nonisolated private static func validateEncodedSize(_ data: Data) throws -> Data {
        guard !data.isEmpty else { throw ImagePasteDataError.missingImage }
        guard data.count <= maximumEncodedByteCount else {
            throw ImagePasteDataError.encodedImageTooLarge
        }
        return data
    }

    nonisolated private static func validatePixelCount(source: CGImageSource) throws {
        guard let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int
        else {
            throw ImagePasteDataError.invalidImage
        }
        try validatePixelCount(width: width, height: height)
    }

    nonisolated static func validatePixelCount(width: Int, height: Int) throws {
        guard width > 0, height > 0 else {
            throw ImagePasteDataError.invalidImage
        }
        guard width <= maximumPixelCount / height else {
            throw ImagePasteDataError.pixelCountTooLarge
        }
    }
}
