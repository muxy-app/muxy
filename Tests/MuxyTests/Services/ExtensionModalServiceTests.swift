import Foundation
import MuxyShared
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
        let page = service.page(for: active, query: "", offset: 0, limit: 100)
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
        let args: [String: Any] = ["items": [["id": "a", "title": "Alpha"]]]

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
        let page = service.page(for: active, query: "", offset: 0, limit: 100)
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

    @Test("page windows the dataset and reports hasMore")
    func pageWindowsDataset() {
        let service = ExtensionModalService()
        let request = makeStreamingRequest(service)
        request.dataset.append((0 ..< 5).map { ExtensionModalService.Item(id: "\($0)", title: "Item \($0)", subtitle: nil) })

        let first = service.page(for: request, query: "", offset: 0, limit: 2)
        #expect(first.items.map(\.id) == ["0", "1"])
        #expect(first.hasMore)

        let last = service.page(for: request, query: "", offset: 4, limit: 2)
        #expect(last.items.map(\.id) == ["4"])
        #expect(!last.hasMore)
    }

    @Test("page filters the dataset natively by query")
    func pageFiltersByQuery() {
        let service = ExtensionModalService()
        let request = makeStreamingRequest(service)
        request.dataset.append([
            ExtensionModalService.Item(id: "1", title: "Login.swift", subtitle: "auth/Login.swift"),
            ExtensionModalService.Item(id: "2", title: "Logout.swift", subtitle: "auth/Logout.swift"),
            ExtensionModalService.Item(id: "3", title: "Main.swift", subtitle: "Main.swift"),
        ])

        let page = service.page(for: request, query: "auth", offset: 0, limit: 100)
        #expect(page.items.map(\.id) == ["1", "2"])
        #expect(!page.hasMore)
    }

    @Test("streaming session feeds the active dataset and resolves on select")
    func streamingSessionFlow() async throws {
        let service = ExtensionModalService()

        let requestID = service.openSession(extensionID: "ext", args: ["placeholder": "Pick"])
        let active = try #require(service.active)
        #expect(active.dataset.loading)

        service.feedSession([ExtensionModalService.Item(id: "x", title: "X", subtitle: nil)])
        service.feedSession([ExtensionModalService.Item(id: "y", title: "Y", subtitle: nil)])
        service.finishSession()
        #expect(!active.dataset.loading)
        #expect(active.dataset.items.map(\.id) == ["x", "y"])

        async let result = service.awaitSelection(requestID: requestID)
        service.select(ExtensionModalService.Item(id: "y", title: "Y", subtitle: nil))
        let selected = await result
        #expect(selected?.id == "y")
        #expect(service.active == nil)
    }

    @Test("onResult callback fires on select")
    func onResultCallbackFires() {
        let service = ExtensionModalService()
        let requestID = service.openSession(extensionID: "ext", args: [:])
        service.finishSession()

        let captured = ResultBox()
        service.onResult(requestID: requestID) { captured.value = $0?.id ?? "nil" }
        service.select(ExtensionModalService.Item(id: "z", title: "Z", subtitle: nil))
        #expect(captured.value == "z")
        #expect(service.active == nil)
    }

    @Test("dismiss delivers nil to the result callback")
    func dismissDeliversNil() {
        let service = ExtensionModalService()
        let requestID = service.openSession(extensionID: "ext", args: [:])
        service.finishSession()

        let captured = ResultBox()
        captured.value = "unset"
        service.onResult(requestID: requestID) { captured.value = $0?.id ?? "nil" }
        service.dismiss()
        #expect(captured.value == "nil")
    }

    @Test("dataset caps at maxItems")
    func datasetCapsAtMax() {
        let dataset = ExtensionModalService.Dataset()
        let huge = (0 ..< (ExtensionModalService.maxItems + 10))
            .map { ExtensionModalService.Item(id: "\($0)", title: "t", subtitle: nil) }
        dataset.append(huge)
        #expect(dataset.items.count == ExtensionModalService.maxItems)
    }

    @Test("parseItems drops invalid entries and clamps text")
    func parseItemsValidates() {
        let parsed = ExtensionModalService.parseItems([
            ["id": "a", "title": "Alpha"],
            ["id": "", "title": "skip"],
            ["title": "no id"],
        ])
        #expect(parsed.map(\.id) == ["a"])
    }

    @Test("append drops duplicate ids across batches")
    func appendDedupesIDs() {
        let dataset = ExtensionModalService.Dataset()
        dataset.append([
            ExtensionModalService.Item(id: "a", title: "Alpha", subtitle: nil),
            ExtensionModalService.Item(id: "b", title: "Bravo", subtitle: nil),
        ])
        dataset.append([
            ExtensionModalService.Item(id: "a", title: "Alpha dup", subtitle: nil),
            ExtensionModalService.Item(id: "c", title: "Charlie", subtitle: nil),
        ])
        #expect(dataset.items.map(\.id) == ["a", "b", "c"])
    }

    @Test("dismiss by extensionID resolves the active modal with nil")
    func dismissByExtensionDeliversNil() {
        let service = ExtensionModalService()
        let requestID = service.openSession(extensionID: "ext", args: [:])
        service.finishSession()

        let captured = ResultBox()
        captured.value = "unset"
        service.onResult(requestID: requestID) { captured.value = $0?.id ?? "nil" }
        service.dismiss(extensionID: "other")
        #expect(captured.value == "unset")
        service.dismiss(extensionID: "ext")
        #expect(captured.value == "nil")
        #expect(service.active == nil)
    }

    @Test("modal result serialize/parse round-trips the payload")
    func modalResultRoundTrips() throws {
        let payload = Data("{\"id\":\"y\"}".utf8)
        let line = try #require(ExtensionModalResult.serialize(requestID: "ext:1", payload: payload))
        let parsed = try #require(ExtensionModalResult.parse(line))
        #expect(parsed.requestID == "ext:1")
        #expect(parsed.payload == payload)
        #expect(ExtensionModalResult.serialize(requestID: "bad|id", payload: payload) == nil)
    }

    private final class ResultBox {
        var value = ""
    }

    private func makeStreamingRequest(_ service: ExtensionModalService) -> ExtensionModalService.Request {
        service.openSession(extensionID: "ext", args: [:])
        return service.active!
    }

    private func waitForActive(_ service: ExtensionModalService) async throws {
        for _ in 0 ..< 100 {
            if service.active != nil { return }
            try await Task.sleep(for: .milliseconds(1))
        }
        Issue.record("modal never became active")
    }
}
