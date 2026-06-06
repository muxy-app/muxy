import Foundation

@MainActor
@Observable
final class ExtensionModalService {
    static let shared = ExtensionModalService()

    struct Item: Identifiable, Equatable {
        let id: String
        let title: String
        let subtitle: String?
    }

    struct Page: Equatable {
        let items: [Item]
        let hasMore: Bool
    }

    typealias Provider = @MainActor (_ query: String, _ offset: Int, _ limit: Int) async throws -> Page

    enum Source {
        case eager([Item])
        case provider(Provider)
    }

    struct Request: Identifiable, Equatable {
        let id: String
        let extensionID: String
        let placeholder: String
        let emptyLabel: String
        let noMatchLabel: String
        let source: Source

        static func == (lhs: Request, rhs: Request) -> Bool {
            lhs.id == rhs.id
        }
    }

    static let maxItems = 1000
    static let maxTextLength = 200
    static let pageSize = 100
    static let providerAction = "__muxiModalProvider"

    private(set) var active: Request?
    private var continuation: CheckedContinuation<Item?, Never>?
    private var sequence = 0

    func present(extensionID: String, source: Source, args: [String: Any]) async -> Item? {
        sequence += 1
        let request = Request(
            id: "\(extensionID):\(sequence)",
            extensionID: extensionID,
            placeholder: text(args, "placeholder") ?? "Search...",
            emptyLabel: text(args, "emptyLabel") ?? "No items",
            noMatchLabel: text(args, "noMatchLabel") ?? "No matches",
            source: source
        )
        resolve(with: nil)
        return await withCheckedContinuation { continuation in
            self.continuation = continuation
            active = request
        }
    }

    func present(extensionID: String, args: [String: Any]) async throws -> Item? {
        let items = try parseItems(args)
        return await present(extensionID: extensionID, source: .eager(items), args: args)
    }

    func loadPage(for request: Request, query: String, offset: Int, limit: Int) async throws -> Page {
        switch request.source {
        case let .eager(items):
            return await Self.windowed(items: items, query: query, offset: offset, limit: limit)
        case let .provider(provider):
            try Task.checkCancellation()
            let page = try await provider(query, offset, limit)
            try Task.checkCancellation()
            let clamped = page.items.prefix(Self.maxItems).compactMap(clamp)
            return Page(items: Array(clamped), hasMore: page.hasMore)
        }
    }

    private static func windowed(items: [Item], query: String, offset: Int, limit: Int) async -> Page {
        let task = Task.detached(priority: .userInitiated) {
            let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            let filtered = trimmed.isEmpty ? items : items.filter { item in
                item.title.lowercased().contains(trimmed)
                    || (item.subtitle?.lowercased().contains(trimmed) ?? false)
            }
            let window = filtered.dropFirst(offset).prefix(limit)
            return Page(items: Array(window), hasMore: offset + window.count < filtered.count)
        }
        return await withTaskCancellationHandler {
            await task.value
        } onCancel: {
            task.cancel()
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

    func filter(_ query: String, in items: [Item]) -> [Item] {
        let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !trimmed.isEmpty else { return items }
        return items.filter { item in
            item.title.lowercased().contains(trimmed)
                || (item.subtitle?.lowercased().contains(trimmed) ?? false)
        }
    }

    private func resolve(with item: Item?) {
        guard let continuation else { return }
        self.continuation = nil
        active = nil
        continuation.resume(returning: item)
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

    private func clamp(_ item: Item) -> Item? {
        guard let id = clamped(item.id), !id.isEmpty else { return nil }
        guard let title = clamped(item.title), !title.isEmpty else { return nil }
        return Item(id: id, title: title, subtitle: clamped(item.subtitle))
    }

    private func text(_ args: [String: Any], _ key: String) -> String? {
        clamped(args[key] as? String)
    }

    private func clamped(_ value: String?) -> String? {
        guard let value, !value.isEmpty else { return nil }
        return String(value.prefix(Self.maxTextLength))
    }
}

extension ExtensionModalService.Page {
    static func from(_ raw: Any?) -> ExtensionModalService.Page {
        if let array = raw as? [Any] {
            return ExtensionModalService.Page(items: parseItems(array), hasMore: false)
        }
        guard let dict = raw as? [String: Any] else {
            return ExtensionModalService.Page(items: [], hasMore: false)
        }
        let items = parseItems(dict["items"] as? [Any] ?? [])
        return ExtensionModalService.Page(items: items, hasMore: dict["hasMore"] as? Bool ?? false)
    }

    private static func parseItems(_ raw: [Any]) -> [ExtensionModalService.Item] {
        raw.compactMap { entry in
            guard let dict = entry as? [String: Any],
                  let id = dict["id"] as? String, !id.isEmpty,
                  let title = dict["title"] as? String, !title.isEmpty
            else { return nil }
            return ExtensionModalService.Item(id: id, title: title, subtitle: dict["subtitle"] as? String)
        }
    }
}
