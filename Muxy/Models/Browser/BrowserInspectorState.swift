import Foundation

@MainActor
@Observable
final class BrowserInspectorState {
    enum Mode: String {
        case off
        case pick
    }

    var inspectorMode: Mode = .off

    var annotations: [BrowserAnnotation] = []
    var computedStyleSeeds: [UUID: [String: String]] = [:]

    var showsAnnotationsPanel: Bool = false

    @discardableResult
    func addAnnotation(_ annotation: BrowserAnnotation) -> BrowserAnnotation {
        annotations.append(annotation)
        showsAnnotationsPanel = true
        return annotation
    }

    func removeAnnotation(id: UUID) {
        annotations.removeAll { $0.id == id }
        computedStyleSeeds.removeValue(forKey: id)
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
