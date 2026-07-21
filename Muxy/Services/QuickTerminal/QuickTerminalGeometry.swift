import AppKit

enum QuickTerminalGeometry {
    static let defaultSize = NSSize(
        width: CGFloat(QuickTerminalSizePreferences.defaultWidth),
        height: CGFloat(QuickTerminalSizePreferences.defaultHeight)
    )

    static func frame(in visibleFrame: NSRect, preferredSize: NSSize = defaultSize) -> NSRect {
        frame(screenFrame: visibleFrame, visibleFrame: visibleFrame, preferredSize: preferredSize)
    }

    static func frame(
        screenFrame: NSRect,
        visibleFrame: NSRect,
        preferredSize: NSSize = defaultSize
    ) -> NSRect {
        guard screenFrame.width > 0, screenFrame.height > 0,
              visibleFrame.width > 0, visibleFrame.height > 0,
              preferredSize.width > 0, preferredSize.height > 0
        else { return .zero }
        let size = NSSize(
            width: min(preferredSize.width, visibleFrame.width),
            height: min(preferredSize.height, screenFrame.height)
        )
        let centeredX = screenFrame.midX - size.width / 2
        let minimumX = visibleFrame.minX
        let maximumX = max(minimumX, visibleFrame.maxX - size.width)
        return NSRect(
            x: min(max(centeredX, minimumX), maximumX),
            y: screenFrame.maxY - size.height,
            width: size.width,
            height: size.height
        )
    }
}

@MainActor
enum QuickTerminalScreenResolver {
    static func activeScreen(
        mouseLocation: NSPoint = NSEvent.mouseLocation,
        screens: [NSScreen] = NSScreen.screens,
        keyWindowScreen: NSScreen? = NSApp.keyWindow?.screen,
        mainWindowScreen: NSScreen? = NSApp.mainWindow?.screen,
        mainScreen: NSScreen? = NSScreen.main
    ) -> NSScreen? {
        let frames = screens.map(\.frame)
        let keyScreenIndex = keyWindowScreen.flatMap { candidate in
            screens.firstIndex { $0 === candidate }
        }
        let mainWindowScreenIndex = mainWindowScreen.flatMap { candidate in
            screens.firstIndex { $0 === candidate }
        }
        let mainScreenIndex = mainScreen.flatMap { candidate in
            screens.firstIndex { $0 === candidate }
        }
        guard let index = preferredScreenIndex(
            mouseLocation: mouseLocation,
            screenFrames: frames,
            keyScreenIndex: keyScreenIndex,
            mainWindowScreenIndex: mainWindowScreenIndex,
            mainScreenIndex: mainScreenIndex
        )
        else { return nil }
        return screens[index]
    }

    nonisolated static func preferredScreenIndex(
        mouseLocation: NSPoint,
        screenFrames: [NSRect],
        keyScreenIndex: Int?,
        mainWindowScreenIndex: Int?,
        mainScreenIndex: Int?
    ) -> Int? {
        if let mouseScreenIndex = screenFrames.firstIndex(where: { $0.contains(mouseLocation) }) {
            return mouseScreenIndex
        }
        for candidate in [keyScreenIndex, mainWindowScreenIndex, mainScreenIndex] {
            if let candidate, screenFrames.indices.contains(candidate) {
                return candidate
            }
        }
        return screenFrames.indices.first
    }
}
