import AppKit

@MainActor
enum MaterialMask {
    static func image(opacity: Double) -> NSImage? {
        guard opacity > 0, opacity < 1 else { return nil }
        let color = NSColor.white.withAlphaComponent(CGFloat(opacity))
        let image = NSImage(size: NSSize(width: 1, height: 1), flipped: false) { bounds in
            color.setFill()
            NSBezierPath(rect: bounds).fill()
            return true
        }
        image.capInsets = NSEdgeInsets()
        image.resizingMode = .stretch
        return image
    }
}
