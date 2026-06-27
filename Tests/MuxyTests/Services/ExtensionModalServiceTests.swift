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

    @Test("filter applies modal search options")
    func filterAppliesModalSearchOptions() {
        let service = ExtensionModalService()
        let items = [
            ExtensionModalService.Item(id: "a", title: "Alpha", subtitle: "first word"),
            ExtensionModalService.Item(id: "b", title: "alphabet", subtitle: nil),
            ExtensionModalService.Item(id: "c", title: "ALPHA", subtitle: nil),
        ]

        #expect(service.filter("alpha", in: items, options: .init()).map(\.id) == ["a", "b", "c"])
        #expect(service.filter("alpha", in: items, options: .init(caseSensitive: true)).map(\.id) == ["b"])
        #expect(service.filter("alpha", in: items, options: .init(wholeWord: true)).map(\.id) == ["a", "c"])
        #expect(service.filter("^ALPHA$", in: items, options: .init(caseSensitive: true, regex: true)).map(\.id) == ["c"])
        #expect(service.filter("[", in: items, options: .init(regex: true)).isEmpty)
    }

    @Test("regex search rejects an over-length pattern")
    func regexSearchRejectsOverLengthPattern() {
        let service = ExtensionModalService()
        let items = [ExtensionModalService.Item(id: "a", title: "Alpha", subtitle: nil)]
        let pattern = String(repeating: "a", count: ExtensionModalService.maxRegexPatternLength + 1)
        #expect(service.filter(pattern, in: items, options: .init(regex: true)).isEmpty)
    }

    @Test("catastrophic regex stays bounded instead of hanging the caller")
    func catastrophicRegexStaysBounded() {
        let service = ExtensionModalService()
        let items = (0 ..< 200).map {
            ExtensionModalService.Item(id: "\($0)", title: String(repeating: "a", count: 60) + "!", subtitle: nil)
        }
        let start = ContinuousClock.now
        _ = service.filter("(a+)+$", in: items, options: .init(regex: true))
        let elapsed = start.duration(to: .now)
        #expect(elapsed < .seconds(1), "catastrophic regex scan took \(elapsed)")
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

    @Test("first page regex search is bounded when matches are at the front")
    func firstPageRegexSearchIsBoundedWhenMatchesAreAtFront() {
        let service = ExtensionModalService()
        let request = makeStreamingRequest(service)
        let matching = (0 ..< 150).map {
            ExtensionModalService.Item(id: "match-\($0)", title: "Match \($0)", subtitle: nil)
        }
        let nonmatching = (0 ..< 99_850).map {
            ExtensionModalService.Item(id: "miss-\($0)", title: "Unrelated \($0)", subtitle: nil)
        }
        request.dataset.append(matching + nonmatching)

        let start = ContinuousClock.now
        let page = service.page(
            for: request,
            query: "^Match [0-9]+$",
            options: .init(caseSensitive: true, regex: true),
            offset: 0,
            limit: 100
        )
        let elapsed = start.duration(to: .now)

        #expect(page.items.count == 100)
        #expect(page.items.first?.id == "match-0")
        #expect(page.hasMore)
        #expect(elapsed < .milliseconds(150), "first page regex search took \(elapsed)")
    }

    @Test("streaming session keeps rapid feed batches and resolves on select")
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

    @Test("onQueryChange provided sets session state to queryable")
    func onQueryChangeSetsQueryableState() {
        let service = ExtensionModalService()
        var capturedQuery = ""
        var capturedOptions = ExtensionModalSearchOptions()
        _ = service.openSession(
            extensionID: "ext",
            args: ["items": [["id": "a", "title": "Alpha"]]],
            onQueryChange: { query, options in
                capturedQuery = query
                capturedOptions = options
            }
        )
        
        #expect(service.state == .queryable)
        #expect(service.active?.dynamic == true)
        
        service.queryChanged("test query", options: .init(caseSensitive: true, wholeWord: true, regex: true))
        #expect(capturedQuery == "test query")
        #expect(capturedOptions == .init(caseSensitive: true, wholeWord: true, regex: true))
        
        service.finishSession()
        #expect(service.state == .queryable)
        
        service.dismiss()
        #expect(service.state == .finished)
    }

    @Test("search toolbar is shown only when requested")
    func searchToolbarRequiresExplicitRequest() throws {
        let service = ExtensionModalService()

        _ = service.openSession(extensionID: "ext", args: [:])
        #expect(service.active?.searchToolbar == false)

        _ = service.openSession(extensionID: "ext", args: ["searchToolbar": true])
        #expect(service.active?.searchToolbar == true)
    }
    
    @Test("onQueryChange disabled native filtering in page")
    func onQueryChangeDisablesNativeFiltering() throws {
        let service = ExtensionModalService()
        _ = service.openSession(
            extensionID: "ext",
            args: ["items": [["id": "a", "title": "Alpha"]]],
            onQueryChange: { _, _ in }
        )
        
        let active = try #require(service.active)
        service.feedSession([
            ExtensionModalService.Item(id: "1", title: "Login.swift", subtitle: "auth/Login.swift"),
            ExtensionModalService.Item(id: "2", title: "Logout.swift", subtitle: "auth/Logout.swift"),
        ])
        service.finishSession()
        
        let page = service.page(for: active, query: "auth", offset: 0, limit: 100)
        #expect(page.items.map(\.id) == ["1", "2"])
        #expect(!page.hasMore)
    }

    @Test("queryChanged resets queryable dataset before feeding new results")
    func queryChangedResetsQueryableDataset() throws {
        let service = ExtensionModalService()
        _ = service.openSession(
            extensionID: "ext",
            args: ["items": []],
            onQueryChange: { _, _ in
                service.feedSession([
                    ExtensionModalService.Item(id: "new", title: "New result", subtitle: nil),
                ])
                service.finishSession()
            }
        )

        let active = try #require(service.active)
        service.feedSession([
            ExtensionModalService.Item(id: "old", title: "Old result", subtitle: nil),
        ])
        service.finishSession()

        service.queryChanged("needle")

        let page = service.page(for: active, query: "needle", offset: 0, limit: 100)
        #expect(page.items.map(\.id) == ["new"])
        #expect(!page.hasMore)
    }

    @Test("new queryable sessions accept the first query immediately")
    func newQueryableSessionsAcceptFirstQueryImmediately() {
        let service = ExtensionModalService()
        var received: [String] = []
        _ = service.openSession(
            extensionID: "ext",
            args: ["items": []],
            onQueryChange: { query, _ in received.append("first:\(query)") }
        )
        service.queryChanged("검색")

        _ = service.openSession(
            extensionID: "ext",
            args: ["items": []],
            onQueryChange: { query, _ in received.append("second:\(query)") }
        )
        service.queryChanged("한")

        #expect(received == ["first:검색", "second:한"])
    }
    
    @Test("without onQueryChange session state is feeding then finished")
    func withoutOnQueryChangeStateTransitions() {
        let service = ExtensionModalService()
        _ = service.openSession(extensionID: "ext", args: ["items": [["id": "a", "title": "Alpha"]]])
        
        #expect(service.state == .feeding)
        
        service.finishSession()
        #expect(service.state == .finished)
    }
    
    @Test("modal query serialize/parse round-trips search options")
    func modalQueryRoundTripsSearchOptions() throws {
        let line = try #require(ExtensionModalQuery.serialize(
            requestID: "ext:1",
            queryID: 3,
            query: "한글|query",
            options: ["caseSensitive": true, "wholeWord": true, "regex": false]
        ))
        let parsed = try #require(ExtensionModalQuery.parse(line))

        #expect(parsed.requestID == "ext:1")
        #expect(parsed.queryID == 3)
        #expect(parsed.query == "한글|query")
        #expect(parsed.options["caseSensitive"] == true)
        #expect(parsed.options["wholeWord"] == true)
        #expect(parsed.options["regex"] == false)
    }

    @Test("queryChanged sanitizes long queries and null bytes")
    func queryChangedSanitizes() {
        let service = ExtensionModalService()
        var receivedQuery = ""
        _ = service.openSession(
            extensionID: "ext",
            args: ["items": [["id": "a", "title": "Alpha"]]],
            onQueryChange: { query, _ in receivedQuery = query }
        )
        
        let longQuery = String(repeating: "a", count: 1000)
        service.queryChanged(longQuery + "\u{0000}")
        
        #expect(receivedQuery.count == ExtensionModalService.maxQueryLength)
        #expect(!receivedQuery.contains("\u{0000}"))
    }
    
    @Test("isQueryable returns true only for active queryable session")
    func isQueryableChecks() {
        let service = ExtensionModalService()
        let requestID = service.openSession(
            extensionID: "ext",
            args: ["items": [["id": "a", "title": "Alpha"]]],
            onQueryChange: { _, _ in }
        )
        
        #expect(service.isQueryable(requestID))
        #expect(!service.isQueryable("wrong-id"))
        
        service.dismiss()
        #expect(!service.isQueryable(requestID))
    }

    @Test("modal query serialize/parse round-trips the query")
    func modalQueryRoundTrips() throws {
        let line = try #require(ExtensionModalQuery.serialize(requestID: "ext:1", queryID: 3, query: "a|b c"))
        let parsed = try #require(ExtensionModalQuery.parse(line))
        #expect(parsed.requestID == "ext:1")
        #expect(parsed.queryID == 3)
        #expect(parsed.query == "a|b c")
        #expect(ExtensionModalQuery.serialize(requestID: "bad|id", queryID: 1, query: "x") == nil)
        #expect(ExtensionModalQuery.parse("modal-query|ext:1|notanumber|eA==") == nil)
    }

    @Test("modal query serialization omits the options segment when options are empty")
    func modalQueryWireFormatMatchesOptions() throws {
        let withoutOptions = try #require(ExtensionModalQuery.serialize(requestID: "ext:1", queryID: 1, query: "x"))
        #expect(withoutOptions.split(separator: "|", omittingEmptySubsequences: false).count == 4)

        let withOptions = try #require(ExtensionModalQuery.serialize(
            requestID: "ext:1",
            queryID: 1,
            query: "x",
            options: ["regex": true]
        ))
        #expect(withOptions.split(separator: "|", omittingEmptySubsequences: false).count == 5)
    }

    @Test("modal query parse rejects a malformed options segment")
    func modalQueryParseRejectsMalformedOptions() {
        let payload = Data("x".utf8).base64EncodedString()
        #expect(ExtensionModalQuery.parse("modal-query|ext:1|1|\(payload)|not-base64") == nil)

        let nonBoolOptions = Data(#"{"caseSensitive":"yes"}"#.utf8).base64EncodedString()
        #expect(ExtensionModalQuery.parse("modal-query|ext:1|1|\(payload)|\(nonBoolOptions)") == nil)
    }

    @Test("dynamic flag is read from open args")
    func dynamicFlagPlumbed() {
        let service = ExtensionModalService()
        service.openSession(extensionID: "ext", args: [:])
        #expect(service.active?.dynamic == false)
        service.openSession(extensionID: "ext", args: ["dynamic": true])
        #expect(service.active?.dynamic == true)
    }

    @Test("requestQuery resets the dataset and invokes the handler with a new queryID")
    func requestQueryResetsAndInvokes() {
        let service = ExtensionModalService()
        let requestID = service.openSession(extensionID: "ext", args: ["dynamic": true])
        let active = service.active!
        service.feedSession([ExtensionModalService.Item(id: "stale", title: "Stale", subtitle: nil)])
        service.finishSession()
        #expect(active.dataset.items.map(\.id) == ["stale"])

        let captured = QueryBox()
        service.onQueryRequest(requestID: requestID) { captured.id = $0; captured.query = $1; _ = $2 }
        service.requestQuery(query: "hello")

        #expect(captured.id == 1)
        #expect(captured.query == "hello")
        #expect(active.dataset.items.isEmpty)
        #expect(active.dataset.loading)

        service.feedSession([ExtensionModalService.Item(id: "fresh", title: "Fresh", subtitle: nil)], queryID: captured.id)
        service.finishSession(queryID: captured.id)
        #expect(active.dataset.items.map(\.id) == ["fresh"])
        #expect(!active.dataset.loading)
    }

    @Test("requestQuery ignores duplicate query payloads")
    func requestQueryIgnoresDuplicatePayloads() {
        let service = ExtensionModalService()
        service.openSession(extensionID: "ext", args: ["dynamic": true])
        let active = service.active!
        var receivedIDs: [Int] = []
        service.onQueryRequest(requestID: active.id) { queryID, _, _ in receivedIDs.append(queryID) }

        service.requestQuery(query: "hello", options: .init(caseSensitive: true))
        let revision = active.dataset.revision
        service.requestQuery(query: "hello", options: .init(caseSensitive: true))

        #expect(receivedIDs == [1])
        #expect(active.dataset.revision == revision)
    }

    @Test("stale-queryID feed and finish are dropped")
    func staleQueryFeedDropped() {
        let service = ExtensionModalService()
        service.openSession(extensionID: "ext", args: ["dynamic": true])
        let active = service.active!
        service.onQueryRequest(requestID: active.id) { _, _, _ in }

        service.requestQuery(query: "first")
        service.requestQuery(query: "second")

        service.feedSession([ExtensionModalService.Item(id: "old", title: "Old", subtitle: nil)], queryID: 1)
        #expect(active.dataset.items.isEmpty)

        service.feedSession([ExtensionModalService.Item(id: "new", title: "New", subtitle: nil)], queryID: 2)
        #expect(active.dataset.items.map(\.id) == ["new"])

        service.finishSession(queryID: 1)
        #expect(active.dataset.loading)
        service.finishSession(queryID: 2)
        #expect(!active.dataset.loading)
    }

    @Test("a late untagged initial feed is dropped once a dynamic query has started")
    func untaggedInitialFeedDroppedAfterQuery() {
        let service = ExtensionModalService()
        service.openSession(extensionID: "ext", args: ["dynamic": true])
        let active = service.active!
        service.onQueryRequest(requestID: active.id) { _, _, _ in }

        service.requestQuery(query: "typed")

        service.feedSession([ExtensionModalService.Item(id: "initial", title: "Initial", subtitle: nil)])
        service.finishSession()

        #expect(active.dataset.items.isEmpty)
        #expect(active.dataset.loading)
    }

    @Test("requestQuery is a no-op for a non-dynamic modal")
    func requestQueryIgnoredWhenNotDynamic() {
        let service = ExtensionModalService()
        service.openSession(extensionID: "ext", args: [:])
        let active = service.active!
        service.feedSession([ExtensionModalService.Item(id: "a", title: "A", subtitle: nil)])

        let captured = QueryBox()
        service.onQueryRequest(requestID: active.id) { _, _, _ in captured.id = -1 }
        service.requestQuery(query: "x")

        #expect(captured.id == 0)
        #expect(active.dataset.items.map(\.id) == ["a"])
    }

    private final class ResultBox {
        var value = ""
    }

    private final class QueryBox {
        var id = 0
        var query = ""
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
