import Foundation

enum BrowserZoom {
    static let minimum: Double = 0.5
    static let maximum: Double = 3
    static let defaultValue: Double = 1
    static let step: Double = 1.1

    static func zoomIn(_ current: Double) -> Double {
        min(current * step, maximum)
    }

    static func zoomOut(_ current: Double) -> Double {
        max(current / step, minimum)
    }
}
