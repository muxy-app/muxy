import Foundation

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
            haystack = (subtitle.map { "\(title)\n\($0)" } ?? title).lowercased()
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
    }

    struct Request: Identifiable, Equatable {
        let id: String
        let extensionID: String
        let placeholder: String
        let emptyLabel: String
        let noMatchLabel: String
        let dataset: Dataset
        let onQueryChange: ((String) -> Void)?

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
    private var pendingRequestID: String?
    private var bufferedResults: [String: Item?] = [:]
    private var onQueryChangeHandler: ((String) -> Void)?

    @discardableResult
    func openSession(extensionID: String, args: [String: Any], onQueryChange: ((String) -> Void)? = nil) -> String {
        sequence += 1
        let dataset = Dataset()
        let request = Request(
            id: "\(extensionID):\(sequence)",
            extensionID: extensionID,
            placeholder: text(args, "placeholder") ?? "Search...",
            emptyLabel: text(args, "emptyLabel") ?? "No items",
            noMatchLabel: text(args, "noMatchLabel") ?? "No matches",
            dataset: dataset,
            onQueryChange: onQueryChange
        )
        resolve(with: nil)
        bufferedResults.removeAll()
        session = dataset
        active = request
        pendingRequestID = request.id
        state = onQueryChange != nil ? .queryable : .feeding
        onQueryChangeHandler = onQueryChange
        return request.id
    }

    private var lastFeedTimestamp: UInt64 = 0
    static let minFeedIntervalMs: UInt64 = 16

    func feedSession(_ items: [Item]) {
        let now = DispatchTime.now().uptimeNanoseconds / 1_000_000
        if now - lastFeedTimestamp < Self.minFeedIntervalMs {
            return
        }
        lastFeedTimestamp = now
        session?.append(items)
    }

    func finishSession() {
        session?.finish()
        if state == .feeding {
            state = .finished
        }
    }

    static let maxQueryLength = 500
    static let maxQueryChangeFrequencyMs: UInt64 = 50

    private var lastQueryChangeTimestamp: UInt64 = 0

    func queryChanged(_ query: String) {
        guard state == .queryable else { return }
        guard let active else { return }
        let now = DispatchTime.now().uptimeNanoseconds / 1_000_000
        guard now - lastQueryChangeTimestamp >= Self.maxQueryChangeFrequencyMs else { return }
        lastQueryChangeTimestamp = now
        let sanitized = String(query.prefix(Self.maxQueryLength))
            .replacingOccurrences(of: "\u{0000}", with: "")
        onQueryChangeHandler?(sanitized)
        NotificationSocketServer.shared.pushModalQueryChange(
            extensionID: active.extensionID,
            requestID: active.id,
            query: sanitized
        )
    }

    func isQueryable(_ requestID: String) -> Bool {
        active?.id == requestID && state == .queryable
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

    func page(for request: Request, query: String, offset: Int, limit: Int) -> Page {
        if request.onQueryChange != nil {
            let items = request.dataset.items
            let window = items.dropFirst(offset).prefix(limit)
            return Page(items: Array(window), hasMore: offset + window.count < items.count)
        }
        return Self.window(request.dataset.items, query: query, offset: offset, limit: limit)
    }

    private static func window(_ items: [Item], query: String, offset: Int, limit: Int) -> Page {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let filtered = trimmed.isEmpty ? items : items.filter { matches($0, trimmed) }
        let window = filtered.dropFirst(offset).prefix(limit)
        return Page(items: Array(window), hasMore: offset + window.count < filtered.count)
    }

    private static func matches(_ item: Item, _ needle: String) -> Bool {
        item.haystack.contains(needle)
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

    func filter(_ query: String, in items: [Item]) -> [Item] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !trimmed.isEmpty else { return items }
        return items.filter { Self.matches($0, trimmed) }
    }

    private func resolve(with item: Item?) {
        let requestID = pendingRequestID
        active = nil
        session = nil
        pendingRequestID = nil
        onQueryChangeHandler = nil
        state = .finished
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
