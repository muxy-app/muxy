import AppKit

enum QuickTerminalCutoutGeometry {
    nonisolated static func cutoutRect(
        screenFrame: NSRect,
        safeAreaTop: CGFloat,
        leftAuxiliaryWidth: CGFloat?,
        rightAuxiliaryWidth: CGFloat?
    ) -> NSRect? {
        guard let leftAuxiliaryWidth,
              let rightAuxiliaryWidth,
              safeAreaTop > 0,
              screenFrame.width > 0
        else { return nil }
        let cutoutWidth = screenFrame.width - leftAuxiliaryWidth - rightAuxiliaryWidth
        guard cutoutWidth > 0 else { return nil }
        return NSRect(
            x: screenFrame.minX + leftAuxiliaryWidth,
            y: screenFrame.maxY - safeAreaTop,
            width: cutoutWidth,
            height: safeAreaTop
        )
    }

    nonisolated static func collapsedRect(cutoutRect: NSRect, panelFrame: NSRect) -> NSRect {
        NSRect(
            x: cutoutRect.minX - panelFrame.minX,
            y: panelFrame.height - cutoutRect.height,
            width: cutoutRect.width,
            height: cutoutRect.height
        )
    }
}

@MainActor
extension QuickTerminalCutoutGeometry {
    static func cutoutRect(for screen: NSScreen) -> NSRect? {
        cutoutRect(
            screenFrame: screen.frame,
            safeAreaTop: screen.safeAreaInsets.top,
            leftAuxiliaryWidth: screen.auxiliaryTopLeftArea?.width,
            rightAuxiliaryWidth: screen.auxiliaryTopRightArea?.width
        )
    }
}
