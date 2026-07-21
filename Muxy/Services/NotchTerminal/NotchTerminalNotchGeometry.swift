import AppKit

enum NotchTerminalNotchGeometry {
    nonisolated static func notchRect(
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
        let notchWidth = screenFrame.width - leftAuxiliaryWidth - rightAuxiliaryWidth
        guard notchWidth > 0 else { return nil }
        return NSRect(
            x: screenFrame.minX + leftAuxiliaryWidth,
            y: screenFrame.maxY - safeAreaTop,
            width: notchWidth,
            height: safeAreaTop
        )
    }

    nonisolated static func collapsedRect(notchRect: NSRect, panelFrame: NSRect) -> NSRect {
        NSRect(
            x: notchRect.minX - panelFrame.minX,
            y: panelFrame.height - notchRect.height,
            width: notchRect.width,
            height: notchRect.height
        )
    }
}

@MainActor
extension NotchTerminalNotchGeometry {
    static func notchRect(for screen: NSScreen) -> NSRect? {
        notchRect(
            screenFrame: screen.frame,
            safeAreaTop: screen.safeAreaInsets.top,
            leftAuxiliaryWidth: screen.auxiliaryTopLeftArea?.width,
            rightAuxiliaryWidth: screen.auxiliaryTopRightArea?.width
        )
    }

    static func firstNotchedScreen(_ screens: [NSScreen] = NSScreen.screens) -> NSScreen? {
        screens.first { notchRect(for: $0) != nil }
    }
}
