import SwiftUI

#if os(macOS)
import AppKit
#endif

struct ProviderIconView: View {
    enum Style: Equatable {
        case colored
        case monochrome(Color)
    }

    let iconName: String
    let size: CGFloat
    var style: Style = .colored

    var body: some View {
        #if os(macOS)
        if let image = Self.loadProviderImage(named: iconName) {
            switch style {
            case .colored:
                Image(nsImage: image)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: size, height: size)
            case let .monochrome(color):
                Image(nsImage: Self.templateImage(from: image))
                    .renderingMode(.template)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .foregroundStyle(color)
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
    private static func templateImage(from image: NSImage) -> NSImage {
        let template = (image.copy() as? NSImage) ?? image
        template.isTemplate = true
        return template
    }

    private static func loadProviderImage(named name: String) -> NSImage? {
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
