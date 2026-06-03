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

    struct Request: Identifiable, Equatable {
        let id: String
        let extensionID: String
        let placeholder: String
        let emptyLabel: String
        let noMatchLabel: String
        let items: [Item]

        static func == (lhs: Request, rhs: Request) -> Bool {
            lhs.id == rhs.id
        }
    }

    static let maxItems = 1000
    static let maxTextLength = 200

    private(set) var active: Request?
    private var continuation: CheckedContinuation<Item?, Never>?
    private var sequence = 0

    func present(extensionID: String, args: [String: Any]) async throws -> Item? {
        let items = try parseItems(args)
        sequence += 1
        let request = Request(
            id: "\(extensionID):\(sequence)",
            extensionID: extensionID,
            placeholder: text(args, "placeholder") ?? "Search...",
            emptyLabel: text(args, "emptyLabel") ?? "No items",
            noMatchLabel: text(args, "noMatchLabel") ?? "No matches",
            items: items
        )
        resolve(with: nil)
        return await withCheckedContinuation { continuation in
            self.continuation = continuation
            active = request
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
