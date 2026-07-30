import AppKit
import Foundation

@MainActor
final class AppTransparencyService {
    static let shared = AppTransparencyService()

    private var observers: [NSObjectProtocol] = []
    private var appliedTransparent: Bool?

    private init() {}

    func start() {
        guard observers.isEmpty else { return }
        appliedTransparent = resolvedTransparent()
        observers.append(NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: UserDefaults.standard,
            queue: .main
        ) { _ in
            MainActor.assumeIsolated {
                AppTransparencyService.shared.reconcile()
            }
        })
        observers.append(NotificationCenter.default.addObserver(
            forName: .ghosttyConfigurationDidChange,
            object: nil,
            queue: .main
        ) { _ in
            MainActor.assumeIsolated {
                AppTransparencyService.shared.reapplyAfterConfigReload()
            }
        })
        observers.append(NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
            object: nil,
            queue: .main
        ) { _ in
            MainActor.assumeIsolated {
                AppTransparencyService.shared.reconcile()
            }
        })
    }

    private func resolvedTransparent() -> Bool {
        TerminalPaneGhosttyConfig.resolvedAppearance().isTransparent
    }

    private func reconcile() {
        let transparent = resolvedTransparent()
        guard transparent != appliedTransparent else { return }
        appliedTransparent = transparent
        apply(transparent: transparent)
    }

    private func reapplyAfterConfigReload() {
        let transparent = resolvedTransparent()
        appliedTransparent = transparent
        guard transparent else { return }
        apply(transparent: true)
    }

    private func apply(transparent: Bool) {
        for view in TerminalViewRegistry.shared.liveViews {
            guard let surface = view as? any TerminalPaneAppearanceSurface else { continue }
            if transparent {
                surface.applyTerminalPaneConfiguration()
            } else {
                surface.restoreBaseSurfaceConfiguration()
            }
        }
    }
}
