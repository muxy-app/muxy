import Foundation

struct ScrollAnchor: Equatable {
    var line: Int
    var deltaPixels: CGFloat

    init(line: Int = 0, deltaPixels: CGFloat = 0) {
        self.line = line
        self.deltaPixels = deltaPixels
    }
}

@MainActor
extension ScrollAnchor {
    static func atTopOf(line: Int) -> ScrollAnchor {
        ScrollAnchor(line: max(0, line), deltaPixels: 0)
    }

    static func from(pixelY: CGFloat, in heightMap: HeightMap) -> ScrollAnchor {
        guard heightMap.totalLineCount > 0 else { return ScrollAnchor() }
        let clampedY = max(0, min(pixelY, heightMap.totalHeight))
        let location = heightMap.lineAtY(clampedY)
        return ScrollAnchor(line: location.line, deltaPixels: clampedY - location.topY)
    }

    func pixelY(in heightMap: HeightMap) -> CGFloat {
        guard heightMap.totalLineCount > 0 else { return 0 }
        let clampedLine = max(0, min(line, heightMap.totalLineCount - 1))
        return heightMap.heightAbove(line: clampedLine) + deltaPixels
    }

    func clamped(toLineCount lineCount: Int) -> ScrollAnchor {
        guard lineCount > 0 else { return ScrollAnchor() }
        let safeLine = max(0, min(line, lineCount - 1))
        if safeLine == line { return self }
        return ScrollAnchor(line: safeLine, deltaPixels: 0)
    }
}
