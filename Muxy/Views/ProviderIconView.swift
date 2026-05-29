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
        if let image = loadProviderImage(named: iconName) {
            switch style {
            case .colored:
                Image(nsImage: image)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: size, height: size)
            case let .monochrome(color):
                Image(nsImage: templateImage(from: image))
                    .renderingMode(.template)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .foregroundStyle(color)
                    .frame(width: size, height: size)
            }
        } else {
            Image(systemName: "sparkles")
                .font(.system(size: size * 0.8))
                .foregroundStyle(.secondary)
                .frame(width: size, height: size)
        }
        #else
        Image(systemName: "sparkles")
            .font(.system(size: size * 0.8))
            .foregroundStyle(.secondary)
            .frame(width: size, height: size)
        #endif
    }

    #if os(macOS)
    private func templateImage(from image: NSImage) -> NSImage {
        let template = (image.copy() as? NSImage) ?? image
        template.isTemplate = true
        return template
    }
    #endif

    private func loadProviderImage(named name: String) -> NSImage? {
        #if os(macOS)
        if let iconsURL = Bundle.providerIconsURL {
            let fileURL = iconsURL.appendingPathComponent("\(name).svg")
            if let image = NSImage(contentsOf: fileURL) {
                return image
            }
        }

        if let url = Bundle.appResources.url(forResource: name, withExtension: "svg") ??
            Bundle.main.url(forResource: name, withExtension: "svg")
        {
            return NSImage(contentsOf: url)
        }

        if let symbol = NSImage(systemSymbolName: name, accessibilityDescription: nil) {
            return symbol
        }

        return nil
        #else
        nil
        #endif
    }
}
