import Foundation

@MainActor
@Observable
final class RichInputController {
    static let shared = RichInputController()

    var isPanelVisible: Bool = false

    @ObservationIgnored private var states: [WorktreeKey: RichInputState] = [:]

    init() {}

    func state(for key: WorktreeKey) -> RichInputState {
        if let existing = states[key] {
            return existing
        }
        let new = RichInputState()
        if let draft = RichInputDraftStore.shared.draft(for: key) {
            new.apply(draft)
        }
        states[key] = new
        return new
    }

    func existingState(for key: WorktreeKey) -> RichInputState? {
        states[key]
    }

    func prune(validKeys: Set<WorktreeKey>) {
        states = states.filter { validKeys.contains($0.key) }
    }

    func appendMarkdown(_ markdown: String, for key: WorktreeKey) {
        let trimmed = markdown.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let state = state(for: key)
        if state.text.isEmpty {
            state.text = trimmed
        } else {
            let separator = state.text.hasSuffix("\n") ? "" : "\n"
            state.text.append(separator + trimmed)
        }
        state.focusVersion += 1
    }
}
