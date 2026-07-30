import Foundation

struct BackgroundAppearance: Equatable {
    static let transparencyRange = 0 ... 55
    static let blurIntensityRange = 0 ... 100

    let transparency: Int
    let blurIntensity: Int

    init(transparency: Int, blurIntensity: Int) {
        self.transparency = min(
            max(transparency, Self.transparencyRange.lowerBound),
            Self.transparencyRange.upperBound
        )
        self.blurIntensity = min(
            max(blurIntensity, Self.blurIntensityRange.lowerBound),
            Self.blurIntensityRange.upperBound
        )
    }

    var backgroundOpacity: Double {
        Double(100 - transparency) / 100
    }

    var blurFraction: Double {
        Double(blurIntensity) / 100
    }

    var showsBlur: Bool {
        transparency > 0 && blurIntensity > 0
    }

    var isTransparent: Bool {
        transparency > 0
    }

    func resolvingReduceTransparency(_ reduceTransparency: Bool) -> Self {
        guard reduceTransparency else { return self }
        return Self(transparency: 0, blurIntensity: 0)
    }
}
