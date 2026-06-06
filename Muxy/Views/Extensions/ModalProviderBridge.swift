import Foundation

@MainActor
final class ModalProviderBridge {
    private var pending: [Int: CheckedContinuation<ExtensionModalService.Page, Never>] = [:]
    private var sequence = 0

    func nextToken() -> Int {
        sequence += 1
        return sequence
    }

    func awaitPage(token: Int) async -> ExtensionModalService.Page {
        await withCheckedContinuation { continuation in
            pending[token] = continuation
        }
    }

    func resolve(args: [String: Any]) {
        guard let token = args["requestToken"] as? Int ?? (args["requestToken"] as? Double).map(Int.init),
              let continuation = pending.removeValue(forKey: token)
        else { return }
        continuation.resume(returning: ExtensionModalService.Page.from(args))
    }

    func cancelAll() {
        for continuation in pending.values {
            continuation.resume(returning: ExtensionModalService.Page(items: [], hasMore: false))
        }
        pending.removeAll()
    }
}
