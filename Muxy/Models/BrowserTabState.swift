import Foundation

@MainActor
@Observable
final class BrowserTabState: Identifiable {
    enum InspectorMode: String {
        case off
        case annotate
        case style
    }

    static let defaultURL = "about:blank"
    static let minZoom: Double = 0.25
    static let maxZoom: Double = 4.0
    static let zoomStep: Double = 0.1
    static let homeURL = "https://duckduckgo.com"

    let id: UUID
    let projectPath: String

    var pendingURL: String
    var currentURL: String
    var pageTitle: String = ""
    var faviconData: Data?

    var canGoBack: Bool = false
    var canGoForward: Bool = false
    var isLoading: Bool = false
    var estimatedProgress: Double = 0
    var lastErrorMessage: String?

    var zoom: Double = 1.0
    var scrollY: Double = 0

    var inspectorMode: InspectorMode = .off
    var hoveredSelector: String?

    var annotations: [BrowserAnnotation] = []
    var selectedAnnotationID: UUID?
    var draftAnnotationID: UUID?
    var computedStyleSeeds: [UUID: [String: String]] = [:]

    var showsAnnotationsPanel: Bool = false
    var showsBookmarksPopover: Bool = false
    var showsStyleInspector: Bool = false

    var navigationRequestVersion: Int = 0
    var reloadRequestVersion: Int = 0
    var backRequestVersion: Int = 0
    var forwardRequestVersion: Int = 0
    var zoomRequestVersion: Int = 0
    var scrollRestoreRequestVersion: Int = 0
    var pendingScrollRestore: Double?
    var styleOverridesVersion: Int = 0
    var inspectorModeVersion: Int = 0

    init(
        id: UUID = UUID(),
        projectPath: String,
        initialURL: String = BrowserTabState.homeURL,
        scrollY: Double = 0,
        zoom: Double = 1.0
    ) {
        self.id = id
        self.projectPath = projectPath
        pendingURL = initialURL
        currentURL = initialURL
        self.scrollY = scrollY
        self.zoom = zoom
        pendingScrollRestore = scrollY > 0 ? scrollY : nil
    }

    var displayTitle: String {
        if !pageTitle.isEmpty { return pageTitle }
        if let host = URL(string: currentURL)?.host { return host }
        return "Browser"
    }

    var resolvedURL: URL? {
        BrowserURLNormalizer.normalize(pendingURL)
    }

    func requestNavigate(to rawString: String) {
        let trimmed = rawString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        pendingURL = trimmed
        navigationRequestVersion &+= 1
    }

    func requestReload() {
        reloadRequestVersion &+= 1
    }

    func requestBack() {
        backRequestVersion &+= 1
    }

    func requestForward() {
        forwardRequestVersion &+= 1
    }

    func zoomIn() {
        zoom = min(Self.maxZoom, (zoom + Self.zoomStep).rounded(toPlaces: 2))
        zoomRequestVersion &+= 1
    }

    func zoomOut() {
        zoom = max(Self.minZoom, (zoom - Self.zoomStep).rounded(toPlaces: 2))
        zoomRequestVersion &+= 1
    }

    func resetZoom() {
        zoom = 1.0
        zoomRequestVersion &+= 1
    }

    func setInspectorMode(_ mode: InspectorMode) {
        guard mode != inspectorMode else { return }
        inspectorMode = mode
        inspectorModeVersion &+= 1
        if mode != .off {
            showsAnnotationsPanel = true
        }
    }

    @discardableResult
    func addAnnotation(_ annotation: BrowserAnnotation) -> BrowserAnnotation {
        annotations.append(annotation)
        selectedAnnotationID = annotation.id
        draftAnnotationID = annotation.id
        showsAnnotationsPanel = true
        return annotation
    }

    func removeAnnotation(id: UUID) {
        annotations.removeAll { $0.id == id }
        if selectedAnnotationID == id { selectedAnnotationID = nil }
        if draftAnnotationID == id { draftAnnotationID = nil }
    }

    func updateComment(annotationID: UUID, comment: String) {
        guard let index = annotations.firstIndex(where: { $0.id == annotationID }) else { return }
        annotations[index].comment = comment
    }

    func markAnnotationSent(_ id: UUID) {
        guard let index = annotations.firstIndex(where: { $0.id == id }) else { return }
        annotations[index].status = .sent
    }

    func upsertStyleOverride(_ override: StyleOverride, for annotationID: UUID) {
        guard let index = annotations.firstIndex(where: { $0.id == annotationID }) else { return }
        if let existing = annotations[index].styleOverrides.firstIndex(where: {
            $0.selector == override.selector && $0.property == override.property && $0.scope == override.scope
        }) {
            annotations[index].styleOverrides[existing] = override
        } else {
            annotations[index].styleOverrides.append(override)
        }
        styleOverridesVersion &+= 1
    }

    func removeStyleOverride(id: UUID, for annotationID: UUID) {
        guard let index = annotations.firstIndex(where: { $0.id == annotationID }) else { return }
        annotations[index].styleOverrides.removeAll { $0.id == id }
        styleOverridesVersion &+= 1
    }

    func aggregatedStyleOverrides() -> [StyleOverride] {
        annotations.flatMap(\.styleOverrides)
    }

    func handleProgress(_ progress: Double, isLoading: Bool) {
        estimatedProgress = progress
        self.isLoading = isLoading
    }

    func handleNavigationFinished(url: String, title: String, canGoBack: Bool, canGoForward: Bool) {
        currentURL = url
        pendingURL = url
        pageTitle = title
        self.canGoBack = canGoBack
        self.canGoForward = canGoForward
        isLoading = false
        estimatedProgress = 0
        lastErrorMessage = nil
    }

    func handleNavigationFailed(message: String) {
        lastErrorMessage = message
        isLoading = false
        estimatedProgress = 0
    }
}

private extension Double {
    func rounded(toPlaces places: Int) -> Double {
        let multiplier = pow(10.0, Double(places))
        return (self * multiplier).rounded() / multiplier
    }
}
