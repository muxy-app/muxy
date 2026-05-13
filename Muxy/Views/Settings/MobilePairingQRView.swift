import AppKit
import CoreImage
import SwiftUI

struct MobilePairingQRView: View {
    let uriString: String
    let size: CGFloat

    var body: some View {
        Group {
            if let image = Self.makeQRImage(uriString: uriString, size: size) {
                Image(nsImage: image)
                    .interpolation(.none)
                    .resizable()
                    .frame(width: size, height: size)
                    .accessibilityLabel("Muxy pairing QR code")
            } else {
                placeholder
            }
        }
        .background(Color.white)
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.1), lineWidth: 1)
        )
    }

    private var placeholder: some View {
        ZStack {
            Color.white
            Image(systemName: "qrcode")
                .resizable()
                .scaledToFit()
                .padding(size * 0.2)
                .foregroundStyle(.tertiary)
        }
        .frame(width: size, height: size)
    }

    private static func makeQRImage(uriString: String, size: CGFloat) -> NSImage? {
        guard let data = uriString.data(using: .utf8) else { return nil }
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(data, forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")

        guard let baseImage = filter.outputImage else { return nil }
        let extent = baseImage.extent
        guard extent.width > 0, extent.height > 0 else { return nil }

        let scale = max(1, size / extent.width)
        let scaled = baseImage.transformed(by: CGAffineTransform(scaleX: scale, y: scale))

        let context = CIContext(options: nil)
        guard let cgImage = context.createCGImage(scaled, from: scaled.extent) else { return nil }

        let pixelSize = NSSize(width: scaled.extent.width, height: scaled.extent.height)
        return NSImage(cgImage: cgImage, size: pixelSize)
    }
}
