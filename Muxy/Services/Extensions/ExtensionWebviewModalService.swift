import Foundation
import MuxyShared

@MainActor
@Observable
final class ExtensionWebviewModalService {
    static let shared = ExtensionWebviewModalService()

    static let defaultWidth: Double = 480
    static let defaultHeight: Double = 320
    static let minSize: Double = 120
    static let maxWidth: Double = 900
    static let maxHeight: Double = 760
    static let maxResultBytes = 256 * 1024
    static let requestIDPrefix = ExtensionBridgeJS.webviewModalRequestIDPrefix

    struct OpenRequest {
        let extensionID: String
        let entry: String
        let width: Double?
        let height: Double?
        let dismissOnOutsideClick: Bool
        let data: ExtensionJSON?
    }

    struct Request: Identifiable, Equatable {
        let id: String
        let extensionID: String
        let entry: String
        let width: Double
        let height: Double
        let dismissOnOutsideClick: Bool
        let initialData: ExtensionJSON?

        static func == (lhs: Request, rhs: Request) -> Bool {
            lhs.id == rhs.id
        }
    }

    private(set) var active: Request?
    private var sequence = 0
    private var onResolve: ((ExtensionJSON?) -> Void)?
    private var pendingRequestID: String?
    private var bufferedResults: [String: ExtensionJSON?] = [:]

    @discardableResult
    func open(_ open: OpenRequest) -> String {
        sequence += 1
        let request = Request(
            id: "\(Self.requestIDPrefix):\(open.extensionID):\(sequence)",
            extensionID: open.extensionID,
            entry: open.entry,
            width: clampWidth(open.width),
            height: clampHeight(open.height),
            dismissOnOutsideClick: open.dismissOnOutsideClick,
            initialData: open.data
        )
        resolve(with: nil)
        bufferedResults.removeAll()
        active = request
        pendingRequestID = request.id
        ExtensionLifecycleEvents.modalOpened(extensionID: open.extensionID, modalID: request.id)
        return request.id
    }

    func awaitClose(requestID: String) async -> ExtensionJSON? {
        await withCheckedContinuation { continuation in
            onResult(requestID: requestID) { continuation.resume(returning: $0) }
        }
    }

    func onClose(requestID: String, _ handler: @escaping (ExtensionJSON?) -> Void) {
        onResult(requestID: requestID, handler)
    }

    func submit(requestID: String, result: ExtensionJSON?) {
        guard active?.id == requestID else { return }
        resolve(with: result)
    }

    func dismiss() {
        guard let requestID = active?.id else { return }
        requestClose(requestID: requestID)
    }

    func dismiss(requestID: String) {
        guard active?.id == requestID else { return }
        requestClose(requestID: requestID)
    }

    func dismiss(extensionID: String) {
        guard active?.extensionID == extensionID else { return }
        resolve(with: nil)
    }

    func forceClose(instanceID: String) {
        guard active?.id == instanceID else { return }
        resolve(with: nil)
    }

    private func requestClose(requestID: String) {
        guard let request = active, request.id == requestID else { return }
        let surfaceKey = LifecycleSurfaceKey(kind: .modalWebview, instanceID: request.id)
        Task { @MainActor in
            let verdict = await ExtensionSurfaceBridgeRegistry.shared.requestBeforeClose(surfaceKey)
            guard verdict == .allow, self.active?.id == requestID else { return }
            self.resolve(with: nil)
        }
    }

    private func onResult(requestID: String, _ handler: @escaping (ExtensionJSON?) -> Void) {
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

    private func resolve(with result: ExtensionJSON?) {
        let requestID = pendingRequestID
        let extensionID = active?.extensionID
        active = nil
        pendingRequestID = nil
        if let requestID, let extensionID {
            ExtensionLifecycleEvents.modalClosed(extensionID: extensionID, modalID: requestID)
        }
        if let handler = onResolve {
            onResolve = nil
            handler(result)
            return
        }
        guard let requestID else { return }
        bufferedResults[requestID] = result
    }

    private func clampWidth(_ value: Double?) -> Double {
        min(max(value ?? Self.defaultWidth, Self.minSize), Self.maxWidth)
    }

    private func clampHeight(_ value: Double?) -> Double {
        min(max(value ?? Self.defaultHeight, Self.minSize), Self.maxHeight)
    }
}
