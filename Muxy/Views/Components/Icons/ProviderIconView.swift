import SwiftUI

#if os(macOS)
import AppKit
#endif

struct ProviderIconView: View {
    let iconName: String
    let size: CGFloat
    var monochromeTint: Color = MuxyTheme.fg

    var body: some View {
        #if os(macOS)
        if let image = Self.loadProviderImage(named: iconName) {
            if Self.isColorful(named: iconName, image: image) {
                Image(nsImage: image)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: size, height: size)
            } else {
                Image(nsImage: Self.templateImage(named: iconName, image: image))
                    .renderingMode(.template)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .foregroundStyle(monochromeTint)
                    .frame(width: size, height: size)
            }
        } else {
            fallbackSymbol
        }
        #else
        fallbackSymbol
        #endif
    }

    private var fallbackSymbol: some View {
        Image(systemName: "sparkles")
            .font(.system(size: size * 0.85, weight: .semibold))
            .frame(width: size, height: size)
    }

    #if os(macOS)
    private static let colorfulCache = NSCache<NSString, NSNumber>()
    private static let imageCache = NSCache<NSString, NSImage>()
    private static let templateCache = NSCache<NSString, NSImage>()

    private static func templateImage(named name: String, image: NSImage) -> NSImage {
        if let cached = templateCache.object(forKey: name as NSString) {
            return cached
        }
        let template = (image.copy() as? NSImage) ?? image
        template.isTemplate = true
        templateCache.setObject(template, forKey: name as NSString)
        return template
    }

    private static func isColorful(named name: String, image: NSImage) -> Bool {
        if let cached = colorfulCache.object(forKey: name as NSString) {
            return cached.boolValue
        }
        let result = imageContainsColor(image)
        colorfulCache.setObject(NSNumber(value: result), forKey: name as NSString)
        return result
    }

    private static func imageContainsColor(_ image: NSImage) -> Bool {
        let dimension = 24
        guard let rep = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: dimension,
            pixelsHigh: dimension,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        )
        else { return false }

        let context = NSGraphicsContext(bitmapImageRep: rep)
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = context
        image.draw(in: NSRect(x: 0, y: 0, width: dimension, height: dimension))
        NSGraphicsContext.restoreGraphicsState()

        for x in 0 ..< dimension {
            for y in 0 ..< dimension {
                guard let color = rep.colorAt(x: x, y: y), color.alphaComponent > 0.1 else { continue }
                let red = color.redComponent
                let green = color.greenComponent
                let blue = color.blueComponent
                if abs(red - green) > 0.08 || abs(green - blue) > 0.08 || abs(red - blue) > 0.08 {
                    return true
                }
            }
        }
        return false
    }

    private static func loadProviderImage(named name: String) -> NSImage? {
        if let cached = imageCache.object(forKey: name as NSString) {
            return cached
        }
        guard let image = decodeProviderImage(named: name) else { return nil }
        imageCache.setObject(image, forKey: name as NSString)
        return image
    }

    private static func decodeProviderImage(named name: String) -> NSImage? {
        if let iconsURL = Bundle.providerIconsURL {
            let fileURL = iconsURL.appendingPathComponent("\(name).svg")
            if let image = NSImage(contentsOf: fileURL) {
                return image
            }
        }
        if let url = Bundle.appResources.url(forResource: name, withExtension: "svg", subdirectory: "ProviderIcons")
            ?? Bundle.appResources.url(forResource: name, withExtension: "svg")
        {
            return NSImage(contentsOf: url)
        }
        return nil
    }
    #endif
}
