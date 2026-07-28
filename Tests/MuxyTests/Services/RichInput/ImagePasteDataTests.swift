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
