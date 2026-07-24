import AppKit

@MainActor
protocol QuickTerminalSurface: AnyObject {
    var quickTerminalView: NSView { get }
    var onProcessExit: (() -> Void)? { get set }

    func applyQuickTerminalConfiguration()
    func setVisible(_ visible: Bool)
    func setFocused(_ focused: Bool)
    func notifySurfaceUnfocused()
    func tearDown()
}

extension GhosttyTerminalNSView: QuickTerminalSurface {
    var quickTerminalView: NSView { self }

    func applyQuickTerminalConfiguration() {
        setSurfaceConfigurationOverlay { surface in
            QuickTerminalGhosttyConfig.apply(to: surface)
        }
    }
}

@MainActor
final class QuickTerminalSession {
    typealias SurfaceFactory = @MainActor (String) -> any QuickTerminalSurface

    var onProcessExit: (() -> Void)?

    private let homeDirectory: () -> URL
    private let surfaceFactory: SurfaceFactory
    private var surface: (any QuickTerminalSurface)?
    private var isTerminated = false

    init(
        homeDirectory: @escaping () -> URL = { FileManager.default.homeDirectoryForCurrentUser },
        surfaceFactory: @escaping SurfaceFactory = { workingDirectory in
            GhosttyTerminalNSView(workingDirectory: workingDirectory)
        }
    ) {
        self.homeDirectory = homeDirectory
        self.surfaceFactory = surfaceFactory
    }

    var currentSurface: (any QuickTerminalSurface)? { surface }

    func surfaceForPresentation() -> (any QuickTerminalSurface)? {
        guard !isTerminated else { return nil }
        if let surface {
            return surface
        }
        let surface = surfaceFactory(homeDirectory().path)
        let identifier = ObjectIdentifier(surface)
        surface.onProcessExit = { [weak self] in
            self?.handleProcessExit(identifier: identifier)
        }
        self.surface = surface
        surface.applyQuickTerminalConfiguration()
        return surface
    }

    func markVisible(_ visible: Bool) {
        surface?.setVisible(visible)
        surface?.setFocused(visible)
        if !visible {
            surface?.notifySurfaceUnfocused()
        }
    }

    func reloadConfiguration() {
        surface?.applyQuickTerminalConfiguration()
    }

    func terminate() {
        guard !isTerminated else { return }
        isTerminated = true
        releaseSurface()
    }

    private func handleProcessExit(identifier: ObjectIdentifier) {
        guard let surface, ObjectIdentifier(surface) == identifier else { return }
        releaseSurface()
        onProcessExit?()
    }

    private func releaseSurface() {
        guard let surface else { return }
        self.surface = nil
        surface.onProcessExit = nil
        surface.tearDown()
    }
}
