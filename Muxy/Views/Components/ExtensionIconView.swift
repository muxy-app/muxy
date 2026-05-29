import AppKit
import SwiftUI

struct ExtensionIconView: View {
    let icon: ExtensionIcon
    let muxyExtension: MuxyExtension
    var size: CGFloat = 12
    var weight: Font.Weight = .semibold

    var body: some View {
        switch icon {
        case let .symbol(name):
            Image(systemName: name)
                .font(.system(size: UIMetrics.scaled(size), weight: weight))
        case let .svg(path):
            svgImage(path: path)
        }
    }

    @ViewBuilder
    private func svgImage(path: String) -> some View {
        if let url = muxyExtension.resolveResource(path),
           let nsImage = ExtensionIconAssetCache.shared.image(extensionID: muxyExtension.id, url: url)
        {
            Image(nsImage: nsImage)
                .resizable()
                .renderingMode(.template)
                .aspectRatio(contentMode: .fit)
                .frame(width: UIMetrics.scaled(size), height: UIMetrics.scaled(size))
        } else {
            Image(systemName: "puzzlepiece.extension")
                .font(.system(size: UIMetrics.scaled(size), weight: weight))
        }
    }
}
