import AppKit
import ImageIO
import Testing
import UniformTypeIdentifiers

@testable import Muxy

@Suite("Image paste data")
@MainActor
struct ImagePasteDataTests {
    @Test("reads PNG image data from a pasteboard")
    func readsPNGData() async throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let pngData = try #require(makeImageData(using: .png))
        pasteboard.clearContents()
        pasteboard.setData(pngData, forType: .png)

        #expect(ImagePasteData.hasImage(in: pasteboard))
        let sourceData = try ImagePasteData.sourceData(from: pasteboard)
        #expect(try await ImagePasteData.pngData(from: sourceData) == pngData)
    }

    @Test("reads dynamically supported image data from a pasteboard")
    func readsDynamicallySupportedData() async throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let bmpData = try #require(makeImageData(using: .bmp))
        let bmpType = NSPasteboard.PasteboardType(UTType.bmp.identifier)
        pasteboard.clearContents()
        pasteboard.setData(bmpData, forType: bmpType)

        #expect(ImagePasteData.hasImage(in: pasteboard))
        let sourceData = try ImagePasteData.sourceData(from: pasteboard)
        let pngData = try await ImagePasteData.pngData(from: sourceData)
        let imageSource = try #require(CGImageSourceCreateWithData(pngData as CFData, nil))
        #expect(CGImageSourceGetType(imageSource) as String? == UTType.png.identifier)
    }

    @Test("text remains preferred when a pasteboard exposes text and image data")
    func preservesTextPriority() throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let pngData = try #require(makeImageData(using: .png))
        pasteboard.clearContents()
        pasteboard.setString("plain text", forType: .string)
        pasteboard.setData(pngData, forType: .png)

        #expect(!ImagePasteData.hasImage(in: pasteboard))
        #expect(throws: ImagePasteDataError.self) {
            try ImagePasteData.sourceData(from: pasteboard)
        }
    }

    @Test("rejects encoded images above the configured limit")
    func rejectsOversizedEncodedImage() async {
        let data = Data(count: ImagePasteData.maximumEncodedByteCount + 1)

        await #expect(throws: ImagePasteDataError.self) {
            try await ImagePasteData.pngData(from: data)
        }
    }

    @Test("rejects images above the configured pixel limit")
    func rejectsOversizedPixelCount() {
        #expect(throws: ImagePasteDataError.self) {
            try ImagePasteData.validatePixelCount(width: 8_001, height: 8_000)
        }
    }

    @Test("rejects invalid image data")
    func rejectsInvalidImage() async {
        await #expect(throws: ImagePasteDataError.self) {
            try await ImagePasteData.pngData(from: Data("not an image".utf8))
        }
    }

    @Test("reads and normalizes a regular image file")
    func readsRegularImageFile() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("image.bmp")
        let bmpData = try #require(makeImageData(using: .bmp))
        try bmpData.write(to: url)

        let pngData = try await ImagePasteData.pngData(contentsOf: url)
        let imageSource = try #require(CGImageSourceCreateWithData(pngData as CFData, nil))

        #expect(CGImageSourceGetType(imageSource) as String? == UTType.png.identifier)
    }

    @Test("rejects an oversized regular file before decoding")
    func rejectsOversizedRegularFile() async throws {
        let directory = try temporaryDirectory()
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("oversized.png")
        #expect(FileManager.default.createFile(atPath: url.path, contents: nil))
        let handle = try FileHandle(forWritingTo: url)
        try handle.truncate(atOffset: UInt64(ImagePasteData.maximumEncodedByteCount + 1))
        try handle.close()

        await #expect(throws: RegularFileReadError.self) {
            try await ImagePasteData.pngData(contentsOf: url)
        }
    }

    private func temporaryDirectory() throws -> URL {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(
            at: directory,
            withIntermediateDirectories: false
        )
        return directory
    }

    private func makeImageData(using fileType: NSBitmapImageRep.FileType) -> Data? {
        guard let representation = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: 1,
            pixelsHigh: 1,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 4,
            bitsPerPixel: 32
        )
        else { return nil }
        return representation.representation(using: fileType, properties: [:])
    }
}
