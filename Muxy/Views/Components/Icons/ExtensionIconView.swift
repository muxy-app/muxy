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
            symbolImage(name)
        case let .svg(path):
            svgImage(path: path)
        }
    }

    private var glyphSize: CGFloat { UIMetrics.scaled(size) }

    private func symbolImage(_ name: String) -> some View {
        Image(systemName: name).resizable().scaledToFit()
            .frame(width: glyphSize, height: glyphSize)
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
                .frame(width: glyphSize, height: glyphSize)
        } else {
            symbolImage("puzzlepiece.extension")
        }
    }
}
