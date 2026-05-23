import Foundation

@MainActor
@Observable
final class BrowserSession: Identifiable {
    static let defaultURL = "about:blank"
    static let homeURL = "https://www.google.com"
    static let minZoom: Double = 0.25
    static let maxZoom: Double = 4.0
    static let zoomStep: Double = 0.1

    let id: UUID
    let projectPath: String
    let nav: BrowserNavigationState
    let inspector: BrowserInspectorState
    let commands: BrowserCommandBus

    init(
        id: UUID = UUID(),
        projectPath: String,
        initialURL: String = BrowserSession.homeURL,
        scrollY: Double = 0,
        zoom: Double = 1.0
    ) {
        self.id = id
        self.projectPath = projectPath
        nav = BrowserNavigationState(initialURL: initialURL, scrollY: scrollY, zoom: zoom)
        inspector = BrowserInspectorState()
        commands = BrowserCommandBus()
    }

    func requestNavigate(to rawString: String) {
        let trimmed = rawString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        nav.pendingURL = trimmed
        guard let url = nav.resolvedURL, BrowserURLNormalizer.isAllowedNavigationURL(url) else { return }
        commands.send(.navigate(url))
    }

    func requestReload() {
        commands.send(.reload)
    }

    func requestStop() {
        nav.isLoading = false
        nav.estimatedProgress = 0
        commands.send(.stop)
    }

    func requestBack() {
        commands.send(.back)
    }

    func requestForward() {
        commands.send(.forward)
    }

    func zoomIn() {
        nav.zoom = min(Self.maxZoom, (nav.zoom + Self.zoomStep).rounded(toPlaces: 2))
        commands.send(.setZoom(nav.zoom))
    }

    func zoomOut() {
        nav.zoom = max(Self.minZoom, (nav.zoom - Self.zoomStep).rounded(toPlaces: 2))
        commands.send(.setZoom(nav.zoom))
    }

    func resetZoom() {
        nav.zoom = 1.0
        commands.send(.setZoom(nav.zoom))
    }

    func setInspectorMode(_ mode: BrowserInspectorState.Mode) {
        guard mode != inspector.inspectorMode else { return }
        inspector.inspectorMode = mode
        if mode != .off {
            inspector.showsAnnotationsPanel = true
        }
        commands.send(.setInspectorMode(mode))
    }

    func upsertStyleOverride(_ override: StyleOverride, for annotationID: UUID) {
        inspector.upsertStyleOverride(override, for: annotationID)
        commands.send(.applyStyleOverrides(inspector.aggregatedStyleOverrides()))
    }

    func removeStyleOverride(id: UUID, for annotationID: UUID) {
        inspector.removeStyleOverride(id: id, for: annotationID)
        commands.send(.applyStyleOverrides(inspector.aggregatedStyleOverrides()))
    }
}

private extension Double {
    func rounded(toPlaces places: Int) -> Double {
        let multiplier = pow(10.0, Double(places))
        return (self * multiplier).rounded() / multiplier
    }
}
