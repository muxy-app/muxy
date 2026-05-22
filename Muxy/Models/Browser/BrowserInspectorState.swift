import Foundation

@MainActor
@Observable
final class BrowserInspectorState {
    enum Mode: String {
        case off
        case annotate
        case style
    }

    var inspectorMode: Mode = .off
    var hoveredSelector: String?

    var annotations: [BrowserAnnotation] = []
    var selectedAnnotationID: UUID?
    var draftAnnotationID: UUID?
    var computedStyleSeeds: [UUID: [String: String]] = [:]

    var showsAnnotationsPanel: Bool = false
    var showsStyleInspector: Bool = false

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
    }

    func removeStyleOverride(id: UUID, for annotationID: UUID) {
        guard let index = annotations.firstIndex(where: { $0.id == annotationID }) else { return }
        annotations[index].styleOverrides.removeAll { $0.id == id }
    }

    func aggregatedStyleOverrides() -> [StyleOverride] {
        annotations.flatMap(\.styleOverrides)
    }
}
