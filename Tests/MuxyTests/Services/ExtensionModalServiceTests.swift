import Foundation
import Testing

@testable import Muxy

@Suite("ExtensionModalService")
@MainActor
struct ExtensionModalServiceTests {
    @Test("present resolves with the selected item")
    func presentResolvesSelection() async throws {
        let service = ExtensionModalService()
        let args: [String: Any] = [
            "items": [
                ["id": "a", "title": "Alpha"],
                ["id": "b", "title": "Beta", "subtitle": "second"],
            ],
        ]

        async let result = service.present(extensionID: "ext", args: args)
        try await waitForActive(service)
        let target = try #require(service.active?.items.last)
        service.select(target)

        let selected = try await result
        #expect(selected?.id == "b")
        #expect(selected?.subtitle == "second")
        #expect(service.active == nil)
    }

    @Test("dismiss resolves with nil")
    func dismissResolvesNil() async throws {
        let service = ExtensionModalService()
        let args: [String: Any] = [["items": [["id": "a", "title": "Alpha"]]]].first!

        async let result = service.present(extensionID: "ext", args: args)
        try await waitForActive(service)
        service.dismiss()

        let selected = try await result
        #expect(selected == nil)
        #expect(service.active == nil)
    }

    @Test("a second modal replaces the first and resolves it with nil")
    func secondModalReplacesFirst() async throws {
        let service = ExtensionModalService()

        async let first = service.present(extensionID: "a", args: ["items": [["id": "1", "title": "First"]]])
        try await waitForActive(service)

        async let second = service.present(extensionID: "b", args: ["items": [["id": "2", "title": "Second"]]])

        let firstResult = try await first
        #expect(firstResult == nil)

        try await waitForActive(service)
        #expect(service.active?.extensionID == "b")

        let target = try #require(service.active?.items.first)
        service.select(target)
        let secondResult = try await second
        #expect(secondResult?.id == "2")
        #expect(service.active == nil)
    }

    @Test("present requires at least one valid item")
    func requiresValidItems() async {
        let service = ExtensionModalService()

        let missingID = await captureError {
            _ = try await service.present(extensionID: "ext", args: ["items": [["title": "no id"]]])
        }
        #expect(missingID is APIError)

        let noItems = await captureError {
            _ = try await service.present(extensionID: "ext", args: [:])
        }
        #expect(noItems is APIError)
    }

    private func captureError(_ operation: () async throws -> Void) async -> Error? {
        do {
            try await operation()
            return nil
        } catch {
            return error
        }
    }

    @Test("filter matches title and subtitle case-insensitively")
    func filterMatchesTitleAndSubtitle() {
        let service = ExtensionModalService()
        let items = [
            ExtensionModalService.Item(id: "a", title: "Open File", subtitle: nil),
            ExtensionModalService.Item(id: "b", title: "Close", subtitle: "Shut the tab"),
        ]

        #expect(service.filter("open", in: items).map(\.id) == ["a"])
        #expect(service.filter("SHUT", in: items).map(\.id) == ["b"])
        #expect(service.filter("  ", in: items).count == 2)
    }

    private func waitForActive(_ service: ExtensionModalService) async throws {
        for _ in 0 ..< 100 {
            if service.active != nil { return }
            try await Task.sleep(for: .milliseconds(1))
        }
        Issue.record("modal never became active")
    }
}
