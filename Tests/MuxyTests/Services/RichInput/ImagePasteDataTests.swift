import AppKit
import Testing

@testable import Muxy

@Suite("Image paste data")
@MainActor
struct ImagePasteDataTests {
    @Test("reads PNG image data from a pasteboard")
    func readsPNGData() throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let pngData = try #require(makePNGData())
        pasteboard.clearContents()
        pasteboard.setData(pngData, forType: .png)

        #expect(ImagePasteData.hasImage(in: pasteboard))
        #expect(ImagePasteData.pngData(from: pasteboard) == pngData)
    }

    @Test("text remains preferred when a pasteboard exposes text and image data")
    func preservesTextPriority() throws {
        let pasteboard = NSPasteboard.withUniqueName()
        defer { pasteboard.releaseGlobally() }
        let pngData = try #require(makePNGData())
        pasteboard.clearContents()
        pasteboard.setString("plain text", forType: .string)
        pasteboard.setData(pngData, forType: .png)

        #expect(!ImagePasteData.hasImage(in: pasteboard))
        #expect(ImagePasteData.pngData(from: pasteboard) == nil)
    }

    private func makePNGData() -> Data? {
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
        return representation.representation(using: .png, properties: [:])
    }
}
