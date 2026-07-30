import AppKit
import Foundation
import GhosttyKit

enum TerminalPaneSurfaceBackground: Equatable {
    case clientThemed
    case transparent
    case opaque

    static func resolve(
        hasClientTheme: Bool,
        appearance: BackgroundAppearance
    ) -> Self {
        guard !hasClientTheme else { return .clientThemed }
        return appearance.isTransparent ? .transparent : .opaque
    }
}

enum TerminalPaneGhosttyConfig {
    @MainActor
    static func apply(to surface: ghostty_surface_t, hasClientTheme: Bool) {
        switch TerminalPaneSurfaceBackground.resolve(
            hasClientTheme: hasClientTheme,
            appearance: resolvedAppearance()
        ) {
        case .clientThemed,
             .opaque:
            return
        case .transparent:
            GhosttyTransparentSurfaceConfig.apply(to: surface)
        }
    }

    @MainActor
    static func resolvedAppearance() -> BackgroundAppearance {
        AppTransparencyPreferences.appearance()
            .resolvingReduceTransparency(
                NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
                    || NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
            )
    }
}

extension GhosttyTerminalNSView: TerminalPaneAppearanceSurface {
    func applyTerminalPaneConfiguration() {
        setSurfaceConfigurationOverlay { [weak self] surface in
            TerminalPaneGhosttyConfig.apply(
                to: surface,
                hasClientTheme: self?.hasActiveClientTheme ?? false
            )
        }
    }
}
