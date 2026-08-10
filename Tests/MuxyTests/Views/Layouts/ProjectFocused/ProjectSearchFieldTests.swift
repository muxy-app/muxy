import AppKit
import SwiftUI
import Testing

@testable import Muxy

@MainActor
@Suite("ProjectSearchField")
struct ProjectSearchFieldTests {
    @Test("renders only when enabled in a wide sidebar")
    func rendersOnlyWhenEnabledAndWide() {
        #expect(textField(in: hostingView(isEnabled: true, isWide: true)) != nil)
        #expect(textField(in: hostingView(isEnabled: false, isWide: true)) == nil)
        #expect(textField(in: hostingView(isEnabled: true, isWide: false)) == nil)
    }

    @Test("clear action clears text")
    func clearActionClearsText() {
        let model = ProjectSearchFieldTestModel(text: "Muxy", isEnabled: true, isWide: true)
        let field = ProjectSearchField(
            text: Binding(
                get: { model.text },
                set: { model.text = $0 }
            ),
            isEnabled: true,
            isWide: true
        )

        field.clear()

        #expect(model.text.isEmpty)
    }

    @Test("disabling search clears the query")
    func disablingSearchClearsQuery() async throws {
        let model = ProjectSearchFieldTestModel(text: "Muxy", isEnabled: true, isWide: true)
        let hostingView = hostingView(model: model)

        model.isEnabled = false

        try await waitUntil {
            hostingView.layoutSubtreeIfNeeded()
            return model.text.isEmpty && self.textField(in: hostingView) == nil
        }
    }

    @Test("collapsing the sidebar preserves the query")
    func collapsingSidebarPreservesQuery() async throws {
        let model = ProjectSearchFieldTestModel(text: "Muxy", isEnabled: true, isWide: true)
        let hostingView = hostingView(model: model)

        model.isWide = false

        try await waitUntil {
            hostingView.layoutSubtreeIfNeeded()
            return self.textField(in: hostingView) == nil
        }
        #expect(model.text == "Muxy")
    }

    private func hostingView(isEnabled: Bool, isWide: Bool) -> NSView {
        let view = ProjectSearchField(text: .constant(""), isEnabled: isEnabled, isWide: isWide)
            .frame(width: 220, height: UIMetrics.controlMedium)
        let hostingView = NSHostingView(rootView: view)
        hostingView.frame = NSRect(x: 0, y: 0, width: 220, height: UIMetrics.controlMedium)
        hostingView.layoutSubtreeIfNeeded()
        return hostingView
    }

    private func hostingView(model: ProjectSearchFieldTestModel) -> NSView {
        let view = ProjectSearchFieldTestHarness(model: model)
            .frame(width: 220, height: UIMetrics.controlMedium)
        let hostingView = NSHostingView(rootView: view)
        hostingView.frame = NSRect(x: 0, y: 0, width: 220, height: UIMetrics.controlMedium)
        hostingView.layoutSubtreeIfNeeded()
        return hostingView
    }

    private func textField(in view: NSView) -> NSTextField? {
        if let field = view as? NSTextField {
            return field
        }
        for subview in view.subviews {
            if let field = textField(in: subview) {
                return field
            }
        }
        return nil
    }

    private func waitUntil(_ condition: @MainActor () -> Bool) async throws {
        for _ in 0..<40 {
            if condition() {
                return
            }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(condition())
    }
}

@MainActor
private final class ProjectSearchFieldTestModel: ObservableObject {
    @Published var text: String
    @Published var isEnabled: Bool
    @Published var isWide: Bool

    init(text: String, isEnabled: Bool, isWide: Bool) {
        self.text = text
        self.isEnabled = isEnabled
        self.isWide = isWide
    }
}

private struct ProjectSearchFieldTestHarness: View {
    @ObservedObject var model: ProjectSearchFieldTestModel

    var body: some View {
        ProjectSearchField(
            text: $model.text,
            isEnabled: model.isEnabled,
            isWide: model.isWide
        )
    }
}
