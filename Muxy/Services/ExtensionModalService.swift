import Foundation

struct ExtensionModalSearchOptions: Equatable {
    var caseSensitive = false
    var wholeWord = false
    var regex = false

    var payload: [String: Bool] {
        [
            "caseSensitive": caseSensitive,
            "wholeWord": wholeWord,
            "regex": regex,
        ]
    }
}

@MainActor
@Observable
final class ExtensionModalService {
    static let shared = ExtensionModalService()

    struct Item: Identifiable, Equatable {
        let id: String
        let title: String
        let subtitle: String?
        let haystack: String

        init(id: String, title: String, subtitle: String?) {
            self.id = id
            self.title = title
            self.subtitle = subtitle
            haystack = subtitle.map { "\(title)\n\($0)" } ?? title
        }
    }

    struct Page: Equatable {
        let items: [Item]
        let hasMore: Bool
    }

    @MainActor
    @Observable
    final class Dataset {
        private(set) var items: [Item] = []
        private(set) var loading = true
        private(set) var revision = 0
        private var seenIDs: Set<String> = []

        func append(_ batch: [Item]) {
            guard !batch.isEmpty else { return }
            var room = ExtensionModalService.maxItems - items.count
            guard room > 0 else { return }
            var added = false
            for item in batch where room > 0 && seenIDs.insert(item.id).inserted {
                items.append(item)
                room -= 1
                added = true
            }
            guard added else { return }
            revision += 1
        }

        func finish() {
            guard loading else { return }
            loading = false
            revision += 1
        }

        func reset() {
            items = []
            seenIDs = []
            loading = true
            revision += 1
        }
    }

    struct Request: Identifiable, Equatable {
        let id: String
        let extensionID: String
        let placeholder: String
        let emptyLabel: String
        let noMatchLabel: String
        let dynamic: Bool
        let searchToolbar: Bool
        let dataset: Dataset

        static func == (lhs: Request, rhs: Request) -> Bool {
            lhs.id == rhs.id
        }
    }

    enum SessionState: Equatable {
        case open
        case feeding
        case queryable
        case finished
    }

    static let maxItems = 100_000
    static let maxTextLength = 200
    static let pageSize = 100

    private(set) var active: Request?
    private(set) var state: SessionState = .finished
    private var sequence = 0
    private var session: Dataset?
    private var onResolve: ((Item?) -> Void)?
    private var onQuery: ((Int, String, ExtensionModalSearchOptions) -> Void)?
    private var queryID = 0
    private var lastQueryPayload: QueryPayload?
    private var callbackQueryID: Int?
    private var pendingRequestID: String?
    private var bufferedResults: [String: Item?] = [:]

    private struct QueryPayload: Equatable {
        let query: String
        let options: ExtensionModalSearchOptions
    }

    @discardableResult
    func openSession(
        extensionID: String,
        args: [String: Any],
        onQueryChange: ((String, ExtensionModalSearchOptions) -> Void)? = nil
    ) -> String {
        sequence += 1
        let dataset = Dataset()
        let isDynamic = ((args["dynamic"] as? Bool) ?? false) || onQueryChange != nil
        let request = Request(
            id: "\(extensionID):\(sequence)",
            extensionID: extensionID,
            placeholder: text(args, "placeholder") ?? "Search...",
            emptyLabel: text(args, "emptyLabel") ?? "No items",
            noMatchLabel: text(args, "noMatchLabel") ?? "No matches",
            dynamic: isDynamic,
            searchToolbar: (args["searchToolbar"] as? Bool) ?? false,
            dataset: dataset
        )
        resolve(with: nil)
        bufferedResults.removeAll()
        onQuery = nil
        queryID = 0
        lastQueryPayload = nil
        session = dataset
        active = request
        pendingRequestID = request.id
        state = isDynamic ? .queryable : .feeding
        if let onQueryChange {
            onQuery = { _, query, options in onQueryChange(query, options) }
        }
        return request.id
    }

    func feedSession(_ items: [Item], queryID: Int? = nil) {
        guard isCurrentQuery(queryID ?? callbackQueryID) else { return }
        session?.append(items)
    }

    func finishSession(queryID: Int? = nil) {
        guard isCurrentQuery(queryID ?? callbackQueryID) else { return }
        session?.finish()
        if state == .feeding {
            state = .finished
        }
    }

    static let maxQueryLength = 500

    func queryChanged(_ query: String, options: ExtensionModalSearchOptions = .init()) {
        requestQuery(query: query, options: options)
    }

    func isQueryable(_ requestID: String) -> Bool {
        active?.id == requestID && state == .queryable
    }

    func onQueryRequest(
        requestID: String,
        _ handler: @escaping (Int, String, ExtensionModalSearchOptions) -> Void
    ) {
        guard active?.id == requestID else { return }
        onQuery = handler
    }

    func requestQuery(query: String, options: ExtensionModalSearchOptions = .init()) {
        guard let request = active, request.dynamic, let handler = onQuery else { return }
        let sanitized = String(query.prefix(Self.maxQueryLength))
            .replacingOccurrences(of: "\u{0000}", with: "")
        let payload = QueryPayload(query: sanitized, options: options)
        guard payload != lastQueryPayload else { return }
        lastQueryPayload = payload
        queryID += 1
        request.dataset.reset()
        let currentQueryID = queryID
        callbackQueryID = currentQueryID
        handler(currentQueryID, sanitized, options)
        callbackQueryID = nil
    }

    private func isCurrentQuery(_ id: Int?) -> Bool {
        (id ?? 0) == queryID
    }

    func onResult(requestID: String, _ handler: @escaping (Item?) -> Void) {
        if let buffered = bufferedResults.removeValue(forKey: requestID) {
            handler(buffered)
            return
        }
        guard active?.id == requestID else {
            handler(nil)
            return
        }
        onResolve = handler
    }

    func awaitSelection(requestID: String) async -> Item? {
        await withCheckedContinuation { continuation in
            onResult(requestID: requestID) { continuation.resume(returning: $0) }
        }
    }

    func present(extensionID: String, args: [String: Any]) async throws -> Item? {
        let items = try parseItems(args)
        let requestID = openSession(extensionID: extensionID, args: args)
        feedSession(items)
        finishSession()
        return await awaitSelection(requestID: requestID)
    }

    func page(
        for request: Request,
        query: String,
        options: ExtensionModalSearchOptions = .init(),
        offset: Int,
        limit: Int
    ) -> Page {
        if request.dynamic {
            let items = request.dataset.items
            let pageOffset = max(offset, 0)
            let pageLimit = max(limit, 0)
            let window = items.dropFirst(pageOffset).prefix(pageLimit)
            return Page(items: Array(window), hasMore: pageOffset + window.count < items.count)
        }
        return Self.window(request.dataset.items, query: query, options: options, offset: offset, limit: limit)
    }

    private static func window(
        _ items: [Item],
        query: String,
        options: ExtensionModalSearchOptions,
        offset: Int,
        limit: Int
    ) -> Page {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let pageOffset = max(offset, 0)
        let pageLimit = max(limit, 0)
        guard !trimmed.isEmpty else {
            let window = items.dropFirst(pageOffset).prefix(pageLimit)
            return Page(items: Array(window), hasMore: pageOffset + window.count < items.count)
        }
        guard let matcher = matcher(for: trimmed, options: options) else { return Page(items: [], hasMore: false) }

        var skipped = 0
        var pageItems: [Item] = []
        pageItems.reserveCapacity(pageLimit)
        for item in items where matcher(item) {
            if skipped < pageOffset {
                skipped += 1
                continue
            }
            guard pageItems.count < pageLimit else {
                return Page(items: pageItems, hasMore: true)
            }
            pageItems.append(item)
        }
        return Page(items: pageItems, hasMore: false)
    }

    static let maxRegexPatternLength = 200
    static let maxRegexScanDuration: TimeInterval = 0.1

    private static func matcher(for needle: String, options: ExtensionModalSearchOptions) -> ((Item) -> Bool)? {
        if options.regex {
            guard needle.count <= maxRegexPatternLength else { return nil }
            let pattern = options.wholeWord ? "(?<![\\p{L}\\p{N}_])(?:\(needle))(?![\\p{L}\\p{N}_])" : needle
            let regexOptions: NSRegularExpression.Options = options.caseSensitive ? [] : [.caseInsensitive]
            guard let expression = try? NSRegularExpression(pattern: pattern, options: regexOptions) else { return nil }
            let deadline = Date().addingTimeInterval(maxRegexScanDuration)
            return { item in regexMatches(expression, in: item.haystack, deadline: deadline) }
        }
        return { item in literalMatches(item.haystack, needle: needle, options: options) }
    }

    private static func regexMatches(
        _ expression: NSRegularExpression,
        in haystack: String,
        deadline: Date
    ) -> Bool {
        guard Date() < deadline else { return false }
        let range = NSRange(haystack.startIndex ..< haystack.endIndex, in: haystack)
        var matched = false
        expression.enumerateMatches(in: haystack, options: .reportProgress, range: range) { result, _, stop in
            if result != nil {
                matched = true
                stop.pointee = true
                return
            }
            if Date() >= deadline {
                stop.pointee = true
            }
        }
        return matched
    }

    private static func literalMatches(
        _ haystack: String,
        needle: String,
        options: ExtensionModalSearchOptions
    ) -> Bool {
        let compareOptions: String.CompareOptions = options.caseSensitive ? [] : [.caseInsensitive]
        guard options.wholeWord else {
            return haystack.range(of: needle, options: compareOptions) != nil
        }

        var searchRange = haystack.startIndex ..< haystack.endIndex
        while let range = haystack.range(of: needle, options: compareOptions, range: searchRange) {
            if isWholeWord(range, in: haystack) {
                return true
            }
            searchRange = range.upperBound ..< haystack.endIndex
        }
        return false
    }

    private static func isWholeWord(_ range: Range<String.Index>, in haystack: String) -> Bool {
        let startsAtBoundary = range.lowerBound == haystack.startIndex
            || !isWordCharacter(haystack[haystack.index(before: range.lowerBound)])
        let endsAtBoundary = range.upperBound == haystack.endIndex
            || !isWordCharacter(haystack[range.upperBound])
        return startsAtBoundary && endsAtBoundary
    }

    private static func isWordCharacter(_ character: Character) -> Bool {
        character.unicodeScalars.allSatisfy {
            CharacterSet.alphanumerics.contains($0) || $0 == "_"
        }
    }

    func select(_ item: Item) {
        resolve(with: item)
    }

    func dismiss() {
        resolve(with: nil)
    }

    func dismiss(requestID: String) {
        guard active?.id == requestID else { return }
        resolve(with: nil)
    }

    func dismiss(extensionID: String) {
        guard active?.extensionID == extensionID else { return }
        resolve(with: nil)
    }

    func filter(
        _ query: String,
        in items: [Item],
        options: ExtensionModalSearchOptions = .init()
    ) -> [Item] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return items }
        guard let matcher = Self.matcher(for: trimmed, options: options) else { return [] }
        return items.filter(matcher)
    }

    private func resolve(with item: Item?) {
        let requestID = pendingRequestID
        active = nil
        session = nil
        pendingRequestID = nil
        state = .finished
        onQuery = nil
        lastQueryPayload = nil
        callbackQueryID = nil
        if let handler = onResolve {
            onResolve = nil
            handler(item)
            return
        }
        guard let requestID else { return }
        bufferedResults[requestID] = item
    }

    private func parseItems(_ args: [String: Any]) throws -> [Item] {
        guard let raw = args["items"] as? [Any] else {
            throw APIError.invalidArguments("modal requires an items array")
        }
        let items = raw.prefix(Self.maxItems).compactMap(parseItem)
        guard !items.isEmpty else {
            throw APIError.invalidArguments("modal requires at least one valid item")
        }
        return items
    }

    private func parseItem(_ raw: Any) -> Item? {
        guard let dict = raw as? [String: Any] else { return nil }
        return clamp(dict)
    }

    private func clamp(_ dict: [String: Any]) -> Item? {
        guard let id = clamped(dict["id"] as? String), !id.isEmpty else { return nil }
        guard let title = clamped(dict["title"] as? String), !title.isEmpty else { return nil }
        return Item(id: id, title: title, subtitle: clamped(dict["subtitle"] as? String))
    }

    private func text(_ args: [String: Any], _ key: String) -> String? {
        clamped(args[key] as? String)
    }

    private func clamped(_ value: String?) -> String? {
        guard let value, !value.isEmpty else { return nil }
        return String(value.prefix(Self.maxTextLength))
    }
}

extension ExtensionModalService {
    static func modalResultPayload(_ item: Item?) -> Any {
        guard let item else { return NSNull() }
        let payload: [String: Any] = ["id": item.id, "title": item.title, "subtitle": item.subtitle ?? NSNull()]
        return payload
    }

    static func parseItems(_ raw: [Any]) -> [Item] {
        raw.compactMap { entry in
            guard let dict = entry as? [String: Any],
                  let id = dict["id"] as? String, !id.isEmpty,
                  let title = dict["title"] as? String, !title.isEmpty
            else { return nil }
            return Item(
                id: String(id.prefix(maxTextLength)),
                title: String(title.prefix(maxTextLength)),
                subtitle: (dict["subtitle"] as? String).map { String($0.prefix(maxTextLength)) }
            )
        }
    }
}
