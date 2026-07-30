import AppKit
import SwiftUI

enum AppTransparencyMaterial {
    static let material = NSVisualEffectView.Material.underWindowBackground
    static let blendingMode = NSVisualEffectView.BlendingMode.behindWindow
    static let state = NSVisualEffectView.State.active
}

struct AppTransparencyBackground: View {
    @AppStorage(AppTransparencyPreferences.transparencyKey)
    private var transparency = AppTransparencyPreferences.defaultTransparency
    @AppStorage(AppTransparencyPreferences.blurIntensityKey)
    private var blurIntensity = AppTransparencyPreferences.defaultBlurIntensity
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    @Environment(\.colorSchemeContrast) private var colorSchemeContrast

    private var appearance: BackgroundAppearance {
        BackgroundAppearance(transparency: transparency, blurIntensity: blurIntensity)
            .resolvingReduceTransparency(reduceTransparency || colorSchemeContrast == .increased)
    }

    var body: some View {
        ZStack {
            if appearance.showsBlur {
                AppTransparencyVisualEffectView(maskOpacity: appearance.blurFraction)
            }
            MuxyTheme.bg.opacity(appearance.backgroundOpacity)
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

private struct AppTransparencyVisualEffectView: NSViewRepresentable {
    let maskOpacity: Double

    func makeNSView(context _: Context) -> NSVisualEffectView {
        configured(NSVisualEffectView())
    }

    func updateNSView(_ view: NSVisualEffectView, context _: Context) {
        configured(view)
    }

    @discardableResult
    private func configured(_ view: NSVisualEffectView) -> NSVisualEffectView {
        view.material = AppTransparencyMaterial.material
        view.blendingMode = AppTransparencyMaterial.blendingMode
        view.state = AppTransparencyMaterial.state
        view.maskImage = MaterialMask.image(opacity: maskOpacity)
        return view
    }
}
