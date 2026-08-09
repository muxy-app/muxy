import Foundation
import Testing

@testable import Muxy

@MainActor
@Suite("Extensions presentation request")
struct ExtensionsPresentationRequestTests {
    @Test("root request has no browse category")
    func rootRequest() {
        let request = ExtensionsPresentationRequest(
            Notification(name: .openExtensionsModal)
        )

        #expect(request.browseCategory == nil)
        #expect(!request.requiresSettingsHandoff)
    }

    @Test("browse request preserves its category across notifications")
    func browseRequest() {
        let notificationCenter = NotificationCenter()
        let expected = ExtensionsPresentationRequest(
            browseCategory: ExtensionMarketplaceCategory.localization
        )
        let recorder = ExtensionsPresentationRequestRecorder()
        let observer = notificationCenter.addObserver(
            forName: .openExtensionsModal,
            object: nil,
            queue: nil
        ) { notification in
            recorder.record(notification)
        }
        defer { notificationCenter.removeObserver(observer) }

        expected.post(notificationCenter: notificationCenter)

        #expect(recorder.request == expected)
        #expect(expected.requiresSettingsHandoff)
    }

    @Test("browse handoff posts the category to an existing Extensions window")
    func browseHandoff() {
        let notificationCenter = NotificationCenter()
        let expected = ExtensionsPresentationRequest(
            browseCategory: ExtensionMarketplaceCategory.localization
        )
        let recorder = ExtensionsPresentationRequestRecorder()
        let observer = notificationCenter.addObserver(
            forName: .openExtensionBrowse,
            object: nil,
            queue: nil
        ) { notification in
            recorder.record(notification)
        }
        defer { notificationCenter.removeObserver(observer) }

        expected.postBrowse(notificationCenter: notificationCenter)

        #expect(recorder.request == expected)
    }

    @Test("repeated browse requests receive distinct identities")
    func repeatedBrowseRequests() {
        let first = ExtensionsPresentationRequest(
            browseCategory: ExtensionMarketplaceCategory.localization
        )
        let second = ExtensionsPresentationRequest(
            browseCategory: ExtensionMarketplaceCategory.localization
        )

        #expect(first.browseCategory == second.browseCategory)
        #expect(first.id != second.id)
    }
}

private final class ExtensionsPresentationRequestRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var storage: ExtensionsPresentationRequest?

    var request: ExtensionsPresentationRequest? {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    func record(_ notification: Notification) {
        lock.lock()
        defer { lock.unlock() }
        storage = ExtensionsPresentationRequest(notification)
    }
}
