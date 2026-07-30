import AppKit
import Foundation
import GhosttyKit

extension GhosttyTerminalNSView: QuickTerminalSurface {
    var quickTerminalView: NSView { self }

    func applyQuickTerminalConfiguration() {
        setSurfaceConfigurationOverlay { surface in
            GhosttyTransparentSurfaceConfig.apply(to: surface)
        }
    }
}
