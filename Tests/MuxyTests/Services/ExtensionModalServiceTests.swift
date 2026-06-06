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
        let active = try #require(service.active)
        let page = try await service.loadPage(for: active, query: "", offset: 0, limit: 100)
        let target = try #require(page.items.last)
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

        let active = try #require(service.active)
        let page = try await service.loadPage(for: active, query: "", offset: 0, limit: 100)
        let target = try #require(page.items.first)
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

    @Test("eager loadPage windows results and reports hasMore")
    func eagerLoadPagePages() async throws {
        let service = ExtensionModalService()
        let items = (0 ..< 5).map { ExtensionModalService.Item(id: "\($0)", title: "Item \($0)", subtitle: nil) }
        let request = ExtensionModalService.Request(
            id: "ext:1",
            extensionID: "ext",
            placeholder: "",
            emptyLabel: "",
            noMatchLabel: "",
            source: .eager(items)
        )

        let first = try await service.loadPage(for: request, query: "", offset: 0, limit: 2)
        #expect(first.items.map(\.id) == ["0", "1"])
        #expect(first.hasMore)

        let last = try await service.loadPage(for: request, query: "", offset: 4, limit: 2)
        #expect(last.items.map(\.id) == ["4"])
        #expect(!last.hasMore)
    }

    @Test("provider loadPage forwards the query window and clamps items")
    func providerLoadPageForwards() async throws {
        let service = ExtensionModalService()
        let captured = CapturedQuery()
        let request = ExtensionModalService.Request(
            id: "ext:1",
            extensionID: "ext",
            placeholder: "",
            emptyLabel: "",
            noMatchLabel: "",
            source: .provider { query, offset, limit in
                captured.query = query
                captured.offset = offset
                captured.limit = limit
                return ExtensionModalService.Page(
                    items: [ExtensionModalService.Item(id: "x", title: "X", subtitle: nil)],
                    hasMore: true
                )
            }
        )

        let page = try await service.loadPage(for: request, query: "fo", offset: 10, limit: 20)
        #expect(captured.query == "fo")
        #expect(captured.offset == 10)
        #expect(captured.limit == 20)
        #expect(page.items.map(\.id) == ["x"])
        #expect(page.hasMore)
    }

    @Test("provider present resolves with the selected item")
    func providerPresentResolves() async throws {
        let service = ExtensionModalService()
        let source = ExtensionModalService.Source.provider { _, _, _ in
            ExtensionModalService.Page(
                items: [ExtensionModalService.Item(id: "p", title: "Picked", subtitle: nil)],
                hasMore: false
            )
        }

        async let result = service.present(extensionID: "ext", source: source, args: ["placeholder": "Pick"])
        try await waitForActive(service)
        let active = try #require(service.active)
        let page = try await service.loadPage(for: active, query: "", offset: 0, limit: 100)
        let target = try #require(page.items.first)
        service.select(target)

        let selected = await result
        #expect(selected?.id == "p")
        #expect(service.active == nil)
    }

    @Test("Page.from parses items array and hasMore")
    func pageFromParses() {
        let dict: [String: Any] = [
            "items": [
                ["id": "a", "title": "Alpha"],
                ["id": "", "title": "skip"],
                ["title": "no id"],
            ],
            "hasMore": true,
        ]
        let page = ExtensionModalService.Page.from(dict)
        #expect(page.items.map(\.id) == ["a"])
        #expect(page.hasMore)

        let bare = ExtensionModalService.Page.from([["id": "b", "title": "Beta"]])
        #expect(bare.items.map(\.id) == ["b"])
        #expect(!bare.hasMore)
    }

    private final class CapturedQuery: @unchecked Sendable {
        var query = ""
        var offset = -1
        var limit = -1
    }

    private func waitForActive(_ service: ExtensionModalService) async throws {
        for _ in 0 ..< 100 {
            if service.active != nil { return }
            try await Task.sleep(for: .milliseconds(1))
        }
        Issue.record("modal never became active")
    }
}
